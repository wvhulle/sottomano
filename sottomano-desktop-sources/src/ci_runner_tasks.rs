pub mod forgejo_runner_container_names;
pub mod forgejo_runner_journal_repos;
pub mod forgejo_runner_tasks;
pub mod systemd_service_runner_discovery;
pub mod woodpecker_agent_tasks;
pub mod woodpecker_agent_varz;

use std::time::SystemTime;

use bytesize::ByteSize;

use crate::{
    delivery_cadence_drivers::SamplingNeed,
    podman_container_engine::libpod_running_containers::{EngineReadError, RunningContainer},
    system_protocol_clients::{
        http_json_client,
        procfs_process_tree::{ProcessTreeCpuLoad, total_cpu_load},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRepository {
    Named(String),
    NotYetKnown,
}

impl TaskRepository {
    #[must_use]
    pub fn short_name(&self) -> Option<&str> {
        match self {
            TaskRepository::Named(full_name) => full_name.rsplit('/').next(),
            TaskRepository::NotYetKnown => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDeadline {
    NotExposed,
    At(SystemTime),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskWebLog {
    Linked(String),
    NotKnown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskUsage {
    NoContainerNow,
    Measured {
        cpu_load: ProcessTreeCpuLoad,
        resident_memory: ByteSize,
    },
}

impl From<&[&RunningContainer]> for TaskUsage {
    fn from(containers: &[&RunningContainer]) -> Self {
        if containers.is_empty() {
            return TaskUsage::NoContainerNow;
        }
        TaskUsage::Measured {
            cpu_load: total_cpu_load(containers.iter().map(|container| container.cpu_load)),
            resident_memory: containers.iter().fold(ByteSize::b(0), |total, container| {
                total + container.resident_memory
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CiTask {
    pub repository: TaskRepository,
    pub run_label: String,
    pub started_at: SystemTime,
    pub deadline: TaskDeadline,
    pub activities: Vec<String>,
    pub images: Vec<String>,
    pub usage: TaskUsage,
    pub web_log: TaskWebLog,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunnerAccess {
    Complete,
    EngineUnreadable(EngineReadError),
    AgentUnreachable(http_json_client::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CiRunnerTasks {
    pub tasks: Vec<CiTask>,
    pub access: RunnerAccess,
}

impl CiRunnerTasks {
    pub(crate) fn sampling_need(&self) -> SamplingNeed {
        match (self.tasks.is_empty(), &self.access) {
            (true, RunnerAccess::Complete) => SamplingNeed::Settled,
            _ => SamplingNeed::KeepSampling,
        }
    }
}

fn sorted_unique(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut values: Vec<String> = values.collect();
    values.sort();
    values.dedup();
    values
}

fn images_of(containers: &[&RunningContainer]) -> Vec<String> {
    sorted_unique(containers.iter().map(|container| container.image.clone()))
}
