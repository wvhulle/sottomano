use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    env,
    future::ready,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bytesize::ByteSize;
use chrono::DateTime;
use futures::{Stream, StreamExt};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking_poll,
    delivery_cadence_drivers::{PeriodicPolling, SamplingNeed, triggered_deliveries_with_recheck},
    podman_container_engine::{
        libpod_running_containers::{
            ContainerCiIdentity, EngineContainers, EngineReadError, EngineReadout,
            RunningContainer, classified_engine_failure,
        },
        systemd_service_engine_discovery::EngineOwner,
    },
    stream_operators::{
        Availability, RereadSchedule, StreamOperators, reread_on_spaced_trigger_or_deadline,
    },
    system_protocol_clients::{
        http_json_client,
        inotify_file_presence::path_present,
        procfs_process_tree::{CpuTickCheckpoints, ProcessTable},
    },
};

mod docker_events;

const DOCKER_API_ROOT: &str = "http://docker/v1.41";
const DOCKER_POLL_PERIOD: Duration = Duration::from_secs(5);
const ROOTFUL_DOCKER_SOCKET: &str = "/run/docker.sock";
const ROOTLESS_DOCKER_SOCKET: &str = "docker.sock";
const RUNTIME_DIRECTORY_VARIABLE: &str = "XDG_RUNTIME_DIR";
const WOODPECKER_STEP_LABEL: &str = "wp_step";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DockerEngineSocket {
    pub owner: EngineOwner,
    pub socket_path: PathBuf,
}

#[must_use]
pub fn candidate_docker_sockets() -> Vec<DockerEngineSocket> {
    let rootful_socket = DockerEngineSocket {
        owner: EngineOwner::Root,
        socket_path: PathBuf::from(ROOTFUL_DOCKER_SOCKET),
    };
    let rootless_socket =
        env::var_os(RUNTIME_DIRECTORY_VARIABLE).map(|runtime_directory| DockerEngineSocket {
            owner: EngineOwner::YourUser,
            socket_path: PathBuf::from(runtime_directory).join(ROOTLESS_DOCKER_SOCKET),
        });
    [rootful_socket]
        .into_iter()
        .chain(rootless_socket)
        .collect()
}

#[derive(Deserialize)]
struct ListedDockerContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "Created", default)]
    created_unix_seconds: u64,
    #[serde(rename = "Labels", default)]
    labels: Option<HashMap<String, String>>,
    #[serde(rename = "State", default)]
    state: String,
}

impl ListedDockerContainer {
    fn container_name(&self) -> String {
        self.names.first().map_or_else(
            || self.id.chars().take(12).collect(),
            |listed_name| listed_name.trim_start_matches('/').to_owned(),
        )
    }

    fn is_running(&self) -> bool {
        self.state.is_empty() || self.state == "running"
    }

    fn step_label(&self) -> Option<&str> {
        self.labels
            .as_ref()?
            .get(WOODPECKER_STEP_LABEL)
            .map(String::as_str)
    }
}

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
    started_at: Option<SystemTime>,
}

fn parsed_start_time(started_at: &str) -> Option<SystemTime> {
    DateTime::parse_from_rfc3339(started_at)
        .ok()
        .map(SystemTime::from)
}

pub(crate) struct DockerEngineSampler {
    socket_path: PathBuf,
    process_starts_by_id: HashMap<String, ContainerProcessStart>,
    checkpoints_by_id: CpuTickCheckpoints<String>,
}

impl DockerEngineSampler {
    fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            process_starts_by_id: HashMap::new(),
            checkpoints_by_id: CpuTickCheckpoints::default(),
        }
    }

    async fn listed_containers(&self) -> Result<Vec<ListedDockerContainer>, EngineReadError> {
        match http_json_client::get_json_over_unix_socket(
            &self.socket_path,
            &format!("{DOCKER_API_ROOT}/containers/json"),
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
                        started_at: parsed_start_time(&inspected.state.started_at),
                    },
                );
            }
        }
    }

    async fn sample(&mut self) -> Result<EngineContainers, EngineReadError> {
        let listed = self.listed_containers().await?;
        let live_ids: HashSet<String> = listed
            .iter()
            .map(|container| container.id.clone())
            .collect();
        self.process_starts_by_id
            .retain(|container_id, _| live_ids.contains(container_id));
        self.checkpoints_by_id
            .retain_keys(|container_id| live_ids.contains(container_id));
        if listed.is_empty() {
            return Ok(EngineContainers {
                containers: Vec::new(),
            });
        }
        self.learn_process_starts(&listed).await;
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
        let container_name = container.container_name();
        RunningContainer {
            id: container.id.clone(),
            shown_name: container
                .step_label()
                .map_or_else(|| container_name.clone(), str::to_owned),
            container_name,
            image: container.image.clone(),
            started_at: process_start
                .and_then(|process_start| process_start.started_at)
                .unwrap_or(UNIX_EPOCH + Duration::from_secs(container.created_unix_seconds)),
            ci_identity: ContainerCiIdentity::NotWoodpecker,
            cpu_load,
            resident_memory: ByteSize::b(totals.resident_bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockerEngineProbe(pub Availability);

impl SnapshotSource for DockerEngineProbe {
    type Error = Infallible;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Docker engine presence";

    fn delivery_stream(socket_path: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(ready(Ok(
            path_present(socket_path).map(|presence| Ok(DockerEngineProbe(presence)))
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerContainerListing {
    pub container_name: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerEngineListing {
    pub listings: Vec<DockerContainerListing>,
}

impl DockerEngineListing {
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.listings
            .iter()
            .filter(|listing| listing.running)
            .count()
    }
}

impl SnapshotSource for DockerEngineListing {
    type Error = EngineReadError;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Docker container names";

    fn delivery_stream(socket_path: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let listed_socket = socket_path.clone();
        path_present(socket_path).available_while(move || {
            let listed_socket = listed_socket.clone();
            SnapshotDelivery::live_push_deliveries(ready(Ok(reread_on_spaced_trigger_or_deadline(
                docker_events::docker_container_event_triggers(listed_socket.clone()),
                docker_events::TRIGGER_BURST_GAP,
                move || {
                    let listed_socket = listed_socket.clone();
                    async move { listed_docker_names(&listed_socket).await }
                },
                |_: &Result<Self, EngineReadError>| RereadSchedule::WaitForTrigger,
            ))))
        })
    }
}

async fn listed_docker_names(socket_path: &Path) -> Result<DockerEngineListing, EngineReadError> {
    let listed = http_json_client::get_json_over_unix_socket::<Vec<ListedDockerContainer>>(
        socket_path,
        &format!("{DOCKER_API_ROOT}/containers/json"),
    )
    .await;
    match listed {
        Ok(listed) => Ok(DockerEngineListing {
            listings: listed
                .iter()
                .map(|container| DockerContainerListing {
                    container_name: container.container_name(),
                    running: container.is_running(),
                })
                .collect(),
        }),
        Err(fetch_failure) => Err(classified_engine_failure(socket_path, fetch_failure).await),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockerEngineReadout(pub EngineReadout);

impl SnapshotSource for DockerEngineReadout {
    type Error = Infallible;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Docker containers";

    fn delivery_stream(socket_path: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let sampled_socket = socket_path.clone();
        path_present(socket_path).available_while(move || {
            let sampled_socket = sampled_socket.clone();
            let sampler = Arc::new(Mutex::new(DockerEngineSampler::new(sampled_socket.clone())));
            triggered_deliveries_with_recheck(
                docker_events::docker_container_event_triggers(sampled_socket),
                docker_events::TRIGGER_BURST_GAP,
                PeriodicPolling::every(DOCKER_POLL_PERIOD),
                move || {
                    let sampler = sampler.clone();
                    async move {
                        let engine_read = sampler.lock().await.sample().await;
                        Ok(DockerEngineReadout(engine_read.into()))
                    }
                },
                |delivered: &Result<Self, Infallible>| {
                    delivered
                        .as_ref()
                        .map_or(SamplingNeed::KeepSampling, |readout| {
                            readout.0.sampling_need()
                        })
                },
            )
            .repeats_as_unchanged()
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ListedDockerContainer, parsed_start_time};

    #[test]
    fn a_listed_container_drops_the_leading_slash_of_its_name() {
        let listed: ListedDockerContainer = serde_json::from_value(json!({
            "Id": "4f1c0e2a9b7d",
            "Names": ["/grafana"],
            "Image": "grafana/grafana:11.3.0",
            "Created": 1_793_649_940,
            "Labels": {},
        }))
        .unwrap();
        assert_eq!(listed.container_name(), "grafana");
        assert_eq!(listed.step_label(), None);
    }

    #[test]
    fn a_woodpecker_step_is_shown_by_its_step_name() {
        let listed: ListedDockerContainer = serde_json::from_value(json!({
            "Id": "abc",
            "Names": ["/wp_01M6_0_clone"],
            "Image": "woodpeckerci/plugin-git",
            "Created": 1_793_649_940,
            "Labels": {"wp_step": "clone"},
        }))
        .unwrap();
        assert_eq!(listed.step_label(), Some("clone"));
    }

    #[test]
    fn docker_start_times_parse_with_nanoseconds() {
        assert!(parsed_start_time("2026-11-10T17:42:03.123456789Z").is_some());
        assert!(parsed_start_time("0001-01-01T00:00:00Z").is_some());
        assert!(parsed_start_time("").is_none());
    }
}
