use std::{
    collections::{BTreeMap, HashMap},
    convert::Infallible,
    future::ready,
    io,
    path::PathBuf,
    time::SystemTime,
};

use futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use systemd::journal::Journal;

use crate::{
    SnapshotDelivery, SnapshotSource,
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskUsage,
        forgejo_runner_container_names::{ForgejoJobName, parse_forgejo_container_name},
        images_of, sorted_unique,
    },
    podman_container_engine::libpod_running_containers::{
        EngineContainers, RunningContainer, polled_engine_samples,
    },
    stream_operators::StreamOperators,
    system_protocol_clients::journal_follow::{JournalEvent, journal_windows},
    systemd_units_journal::journal_record_fields::LoggingUnit,
};

const SYSLOG_IDENTIFIER_FIELD: &str = "SYSLOG_IDENTIFIER";
const RUNNER_SYSLOG_IDENTIFIER: &str = "forgejo-runner";

#[derive(Debug, Deserialize)]
pub struct ForgejoTaskStartRecord {
    #[serde(rename = "MESSAGE")]
    message: String,
    #[serde(flatten)]
    unit: LoggingUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgejoTaskStart {
    at: SystemTime,
    unit_name: String,
    task_id: u64,
    repository: String,
}

fn parse_task_start_message(message: &str) -> Option<(u64, &str)> {
    let (_, after_task) = message.split_once("task ")?;
    let (task_id, after_repo) = after_task.split_once(" repo is ")?;
    let repository = after_repo.split_whitespace().next()?;
    Some((task_id.parse().ok()?, repository))
}

impl JournalEvent for ForgejoTaskStart {
    type Record = ForgejoTaskStartRecord;

    const NAME: &'static str = "forgejo task start";
    const OPENS_USER_JOURNALS: bool = false;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        journal.match_add(SYSLOG_IDENTIFIER_FIELD, RUNNER_SYSLOG_IDENTIFIER)?;
        Ok(())
    }

    fn admit(record: ForgejoTaskStartRecord, at: SystemTime) -> Option<Self> {
        let (task_id, repository) = parse_task_start_message(&record.message)?;
        Some(ForgejoTaskStart {
            at,
            task_id,
            repository: repository.to_owned(),
            unit_name: record.unit.owning_unit()?.name().to_owned(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

fn forgejo_tasks(
    engine: &EngineContainers,
    repositories_by_task: &HashMap<u64, String>,
) -> Vec<CiTask> {
    let containers_by_task = engine.containers.iter().fold(
        BTreeMap::<u64, Vec<&RunningContainer>>::new(),
        |mut containers_by_task, container| {
            if let Some(parsed) = parse_forgejo_container_name(&container.container_name) {
                containers_by_task
                    .entry(parsed.task_id)
                    .or_default()
                    .push(container);
            }
            containers_by_task
        },
    );
    containers_by_task
        .into_iter()
        .map(|(task_id, containers)| CiTask {
            repository: repositories_by_task
                .get(&task_id)
                .map_or(TaskRepository::NotYetKnown, |repository| {
                    TaskRepository::Named(repository.clone())
                }),
            run_label: task_id.to_string(),
            started_at: containers
                .iter()
                .map(|container| container.started_at)
                .min()
                .unwrap_or(SystemTime::UNIX_EPOCH),
            deadline: TaskDeadline::NotExposed,
            activities: sorted_unique(containers.iter().filter_map(|container| {
                match parse_forgejo_container_name(&container.container_name)?.job_name {
                    ForgejoJobName::Named(job_name) => Some(job_name),
                    ForgejoJobName::NotInContainerName => None,
                }
            })),
            images: images_of(&containers),
            usage: TaskUsage::of_containers(&containers),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForgejoRunnerTasks {
    pub runner: CiRunnerTasks,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForgejoRunnerWatch {
    pub unit_name: String,
    pub engine_socket: PathBuf,
}

enum ForgejoRunnerInput<Sampled> {
    TaskStarts(Vec<ForgejoTaskStart>),
    EngineSampled(Sampled),
}

impl SnapshotSource for ForgejoRunnerTasks {
    type Error = Infallible;
    type StreamInputs = ForgejoRunnerWatch;

    const NAME: &'static str = "Forgejo runner tasks";

    fn delivery_stream(
        watch: ForgejoRunnerWatch,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let task_starts = journal_windows::<ForgejoTaskStart>().map(|window| {
            ForgejoRunnerInput::TaskStarts(window.unwrap_or_else(|failure| {
                tracing::warn!("{failure}");
                Vec::new()
            }))
        });
        let engine_samples =
            polled_engine_samples(watch.engine_socket).map(ForgejoRunnerInput::EngineSampled);
        let unit_name = watch.unit_name;
        stream::select(task_starts, engine_samples)
            .scan(HashMap::<u64, String>::new(), move |repositories, input| {
                let delivery = match input {
                    ForgejoRunnerInput::TaskStarts(starts) => {
                        repositories.extend(
                            starts
                                .into_iter()
                                .filter(|start| start.unit_name == unit_name)
                                .map(|start| (start.task_id, start.repository)),
                        );
                        None
                    }
                    ForgejoRunnerInput::EngineSampled(polled) => {
                        let runner = match polled.engine_read {
                            Ok(engine) => CiRunnerTasks {
                                tasks: forgejo_tasks(&engine, repositories),
                                access: RunnerAccess::Complete,
                            },
                            Err(failure) => CiRunnerTasks {
                                tasks: Vec::new(),
                                access: RunnerAccess::EngineUnreadable(failure),
                            },
                        };
                        Some(polled.poll_tick.stamped(Ok(ForgejoRunnerTasks { runner })))
                    }
                };
                ready(Some(delivery))
            })
            .filter_map(ready)
            .repeats_as_unchanged()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use bytesize::ByteSize;

    use super::*;
    use crate::podman_container_engine::libpod_running_containers::{
        ContainerCiIdentity, ContainerCpuLoad,
    };

    fn container(
        container_name: &str,
        started_seconds: u64,
        memory_bytes: u64,
    ) -> RunningContainer {
        RunningContainer {
            id: container_name.to_owned(),
            container_name: container_name.to_owned(),
            shown_name: container_name.to_owned(),
            image: "docker.io/nixos/nix:latest".to_owned(),
            started_at: UNIX_EPOCH + Duration::from_secs(started_seconds),
            ci_identity: ContainerCiIdentity::NotWoodpecker,
            cpu_load: ContainerCpuLoad::BusyCores(1.0),
            resident_memory: ByteSize::b(memory_bytes),
        }
    }

    #[test]
    fn the_runner_log_line_yields_task_and_repo() {
        let message = "time=\"2026-11-05T19:03:55+01:00\" level=info msg=\"task 11282424 repo is wvhulle/bindrune https://data.forgejo.org https://codeberg.org/\"";
        assert_eq!(
            parse_task_start_message(message),
            Some((11_282_424, "wvhulle/bindrune"))
        );
    }

    #[test]
    fn other_runner_log_lines_are_not_task_starts() {
        assert_eq!(
            parse_task_start_message("failed to fetch task: deadline exceeded"),
            None
        );
        assert_eq!(parse_task_start_message("[poller] launched"), None);
    }

    #[test]
    fn containers_of_one_task_merge_and_unrelated_ones_are_left_out() {
        let engine = EngineContainers {
            containers: vec![
                container("FORGEJO-ACTIONS-TASK-7_WORKFLOW-aa_JOB-build", 200, 100),
                container("FORGEJO-ACTIONS-TASK-7_WORKFLOW-aa_JOB-build_db", 100, 50),
                container("postgres", 1, 999),
            ],
        };
        let repositories = HashMap::from([(7, "wvhulle/bindrune".to_owned())]);
        let tasks = forgejo_tasks(&engine, &repositories);
        assert_eq!(tasks.len(), 1);
        assert_eq!(
            tasks[0].repository,
            TaskRepository::Named("wvhulle/bindrune".to_owned())
        );
        assert_eq!(tasks[0].started_at, UNIX_EPOCH + Duration::from_secs(100));
        assert_eq!(tasks[0].activities, ["build"]);
        assert_eq!(
            tasks[0].usage,
            TaskUsage::Measured {
                cpu_load: ContainerCpuLoad::BusyCores(2.0),
                resident_memory: ByteSize::b(150),
            }
        );
    }

    #[test]
    fn an_unmatched_task_falls_back_to_its_id_and_job() {
        let engine = EngineContainers {
            containers: vec![container(
                "FORGEJO-ACTIONS-TASK-7_WORKFLOW-aa_JOB-build",
                1,
                1,
            )],
        };
        let tasks = forgejo_tasks(&engine, &HashMap::new());
        assert_eq!(tasks[0].repository, TaskRepository::NotYetKnown);
        assert_eq!(tasks[0].run_label, "7");
        assert_eq!(tasks[0].activities, ["build"]);
    }
}
