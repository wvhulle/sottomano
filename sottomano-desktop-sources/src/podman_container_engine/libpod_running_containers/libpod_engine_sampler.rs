use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Instant,
};

use bytesize::ByteSize;

use super::{
    ContainerCiIdentity, ContainerPhase, ContainerSelection, EngineContainers, EngineListing,
    EngineReadError, RunningContainer,
    libpod_listed_containers::{ListedContainer, listed_containers_with_one_retry},
    phased_containers,
};
use crate::{
    blocking_background_work::blocking_poll,
    podman_container_engine::{
        libpod_inspect_ci_identity::{WoodpeckerStepIdentity, fetch_woodpecker_step_identity},
        procfs_conmon_sessions::ConmonSessions,
    },
    system_protocol_clients::{
        procfs_process_tree::{CpuTickCheckpoints, ProcessTable, ProcessTreeTotals},
        sysfs_cgroup_memory::unit_working_set_of_process,
    },
};

pub(crate) struct HostProcesses {
    pub(super) process_table: ProcessTable,
    pub(super) conmon_sessions: ConmonSessions,
    pub(super) cgroup_memory_of: fn(u32) -> Option<ByteSize>,
}

impl HostProcesses {
    fn read_current() -> Option<Self> {
        let process_table = ProcessTable::read_current()?;
        let conmon_sessions = ConmonSessions::read_current(&process_table);
        Some(Self {
            process_table,
            conmon_sessions,
            cgroup_memory_of: unit_working_set_of_process,
        })
    }

    fn container_memory(&self, container: &ListedContainer, totals: ProcessTreeTotals) -> ByteSize {
        u32::try_from(container.pid)
            .inspect_err(|conversion_failure| {
                tracing::debug!(pid = container.pid, %conversion_failure, "container pid is not a process id");
            })
            .ok()
            .and_then(self.cgroup_memory_of)
            .unwrap_or(ByteSize::b(totals.resident_bytes))
    }

    fn container_totals(&self, container: &ListedContainer) -> ProcessTreeTotals {
        self.process_table.forest_totals(
            self.conmon_sessions
                .measured_roots(&container.id, container.pid),
        )
    }
}

pub(crate) struct EngineSampler {
    socket_path: PathBuf,
    identities_by_id: HashMap<String, WoodpeckerStepIdentity>,
    pub(super) checkpoints_by_id: CpuTickCheckpoints<String>,
}

impl EngineSampler {
    pub(crate) fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            identities_by_id: HashMap::new(),
            checkpoints_by_id: CpuTickCheckpoints::default(),
        }
    }

    async fn learn_new_identities(&mut self, listed: &[ListedContainer]) {
        let unknown_step_ids: Vec<String> = listed
            .iter()
            .filter(|container| container.step_label().is_some())
            .filter(|container| !self.identities_by_id.contains_key(&container.id))
            .map(|container| container.id.clone())
            .collect();
        for container_id in unknown_step_ids {
            if let Some(identity) =
                fetch_woodpecker_step_identity(&self.socket_path, &container_id).await
            {
                self.identities_by_id.insert(container_id, identity);
            }
        }
    }

    pub(crate) async fn sample(&mut self) -> Result<EngineContainers, EngineReadError> {
        let listed: Vec<ListedContainer> =
            listed_containers_with_one_retry(&self.socket_path, ContainerSelection::Running)
                .await?
                .into_iter()
                .filter(|listed| listed.phase() == ContainerPhase::Running)
                .collect();
        self.measured_running(listed).await
    }

    pub(super) async fn sample_listing(&mut self) -> Result<EngineListing, EngineReadError> {
        let phased = phased_containers(
            listed_containers_with_one_retry(&self.socket_path, ContainerSelection::All).await?,
            ListedContainer::phase,
            ListedContainer::stopped_container,
        );
        Ok(EngineListing {
            running: self.measured_running(phased.running).await?,
            stopped: phased.stopped,
        })
    }

    async fn measured_running(
        &mut self,
        listed: Vec<ListedContainer>,
    ) -> Result<EngineContainers, EngineReadError> {
        if listed.is_empty() {
            self.identities_by_id.clear();
            self.checkpoints_by_id.clear();
            return Ok(EngineContainers {
                containers: Vec::new(),
            });
        }
        self.learn_new_identities(&listed).await;
        let host_processes = blocking_poll(|| {
            HostProcesses::read_current().ok_or(EngineReadError::ProcessTableUnreadable)
        })
        .await?;
        Ok(self.measured(&listed, &host_processes, Instant::now()))
    }

    pub(super) fn measured(
        &mut self,
        listed: &[ListedContainer],
        host_processes: &HostProcesses,
        taken_at: Instant,
    ) -> EngineContainers {
        let live_ids: HashSet<&str> = listed
            .iter()
            .map(|container| container.id.as_str())
            .collect();
        self.identities_by_id
            .retain(|container_id, _| live_ids.contains(container_id.as_str()));
        self.checkpoints_by_id
            .retain_keys(|container_id| live_ids.contains(container_id.as_str()));
        let containers = listed
            .iter()
            .map(|container| self.measured_container(container, host_processes, taken_at))
            .collect();
        EngineContainers { containers }
    }

    fn measured_container(
        &mut self,
        container: &ListedContainer,
        host_processes: &HostProcesses,
        taken_at: Instant,
    ) -> RunningContainer {
        let totals = host_processes.container_totals(container);
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
            started_at: container.started_at(),
            ci_identity: self
                .identities_by_id
                .get(&container.id)
                .map_or(ContainerCiIdentity::NotWoodpecker, |identity| {
                    ContainerCiIdentity::WoodpeckerStep(identity.clone())
                }),
            cpu_load,
            resident_memory: host_processes.container_memory(container, totals),
        }
    }
}
