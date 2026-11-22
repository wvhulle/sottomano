use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant, UNIX_EPOCH},
};

use bytesize::ByteSize;
use serde::Deserialize;

use super::{DOCKER_API_ROOT, ListedDockerContainer};
use crate::{
    blocking_background_work::blocking_poll,
    container_runtime_facts::ContainerStartTime,
    podman_container_engine::libpod_running_containers::{
        ContainerCiIdentity, EngineContainers, EngineListing, EngineReadError, RunningContainer,
        classified_engine_failure, parsed_engine_timestamp, phased_containers,
    },
    system_protocol_clients::{
        http_json_client,
        procfs_process_tree::{CpuTickCheckpoints, ProcessTable},
        sysfs_cgroup_memory::unit_working_set_of_process,
    },
};

#[derive(Deserialize)]
struct InspectedContainerState {
    #[serde(rename = "Pid", default)]
    pid: i32,
    #[serde(rename = "StartedAt", default)]
    started_at: String,
}

#[derive(Deserialize)]
struct InspectedDockerContainer {
    #[serde(rename = "State")]
    state: InspectedContainerState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContainerProcessStart {
    pid: i32,
    started_at: ContainerStartTime,
}

pub(crate) struct DockerEngineSampler {
    socket_path: PathBuf,
    process_starts_by_id: HashMap<String, ContainerProcessStart>,
    checkpoints_by_id: CpuTickCheckpoints<String>,
}

impl DockerEngineSampler {
    pub(super) fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            process_starts_by_id: HashMap::new(),
            checkpoints_by_id: CpuTickCheckpoints::default(),
        }
    }

    async fn listed_containers(&self) -> Result<Vec<ListedDockerContainer>, EngineReadError> {
        match http_json_client::get_json_over_unix_socket(
            &self.socket_path,
            &format!("{DOCKER_API_ROOT}/containers/json?all=1"),
        )
        .await
        {
            Ok(listed) => Ok(listed),
            Err(fetch_failure) => {
                Err(classified_engine_failure(&self.socket_path, fetch_failure).await)
            }
        }
    }

    async fn learn_process_starts(&mut self, listed: &[ListedDockerContainer]) {
        let unknown_ids: Vec<String> = listed
            .iter()
            .filter(|container| !self.process_starts_by_id.contains_key(&container.id))
            .map(|container| container.id.clone())
            .collect();
        for container_id in unknown_ids {
            let inspected: Result<InspectedDockerContainer, _> =
                http_json_client::get_json_over_unix_socket(
                    &self.socket_path,
                    &format!("{DOCKER_API_ROOT}/containers/{container_id}/json"),
                )
                .await;
            if let Ok(inspected) = inspected
                && inspected.state.pid > 0
            {
                self.process_starts_by_id.insert(
                    container_id,
                    ContainerProcessStart {
                        pid: inspected.state.pid,
                        started_at: parsed_engine_timestamp(&inspected.state.started_at).into(),
                    },
                );
            }
        }
    }

    pub(super) async fn sample(&mut self) -> Result<EngineListing, EngineReadError> {
        let phased = phased_containers(
            self.listed_containers().await?,
            ListedDockerContainer::phase,
            ListedDockerContainer::stopped_container,
        );
        Ok(EngineListing {
            running: self.measured_running(&phased.running).await?,
            stopped: phased.stopped,
        })
    }

    async fn measured_running(
        &mut self,
        listed: &[ListedDockerContainer],
    ) -> Result<EngineContainers, EngineReadError> {
        let running_ids: HashSet<String> = listed
            .iter()
            .map(|container| container.id.clone())
            .collect();
        self.process_starts_by_id
            .retain(|container_id, _| running_ids.contains(container_id));
        self.checkpoints_by_id
            .retain_keys(|container_id| running_ids.contains(container_id));
        if listed.is_empty() {
            return Ok(EngineContainers {
                containers: Vec::new(),
            });
        }
        self.learn_process_starts(listed).await;
        let process_table = blocking_poll(|| {
            ProcessTable::read_current().ok_or(EngineReadError::ProcessTableUnreadable)
        })
        .await?;
        let taken_at = Instant::now();
        let containers = listed
            .iter()
            .map(|container| self.measured_container(container, &process_table, taken_at))
            .collect();
        Ok(EngineContainers { containers })
    }

    fn measured_container(
        &mut self,
        container: &ListedDockerContainer,
        process_table: &ProcessTable,
        taken_at: Instant,
    ) -> RunningContainer {
        let process_start = self.process_starts_by_id.get(&container.id).copied();
        let totals = process_start
            .map(|process_start| process_table.tree_totals(process_start.pid))
            .unwrap_or_default();
        let cpu_load = self.checkpoints_by_id.load_since_previous(
            container.id.clone(),
            totals.spent_ticks,
            taken_at,
        );
        RunningContainer {
            id: container.id.clone(),
            shown_name: container.shown_name(),
            container_name: container.container_name(),
            image: container.image.clone(),
            started_at: match process_start
                .map_or(ContainerStartTime::Unrecorded, |process_start| {
                    process_start.started_at
                }) {
                ContainerStartTime::StartedAt(started_at) => started_at,
                ContainerStartTime::Unrecorded => {
                    UNIX_EPOCH + Duration::from_secs(container.created_unix_seconds)
                }
            },
            ci_identity: ContainerCiIdentity::NotWoodpecker,
            cpu_load,
            resident_memory: process_start
                .and_then(|process_start| {
                    u32::try_from(process_start.pid)
                        .inspect_err(|conversion_failure| {
                            tracing::debug!(pid = process_start.pid, %conversion_failure, "container pid is not a process id");
                        })
                        .ok()
                })
                .and_then(unit_working_set_of_process)
                .unwrap_or(ByteSize::b(totals.resident_bytes)),
        }
    }
}
