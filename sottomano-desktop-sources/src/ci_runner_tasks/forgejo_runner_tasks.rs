use std::{
    collections::{BTreeMap, HashMap},
    future::ready,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use futures::{Stream, StreamExt, stream};

use crate::{
    SnapshotDelivery, SnapshotSource,
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskStopControl,
        TaskUsage, TaskWebLog,
        forgejo_runner_container_names::{ForgejoJobName, parse_forgejo_container_name},
        forgejo_runner_journal_repos::ForgejoTaskStart,
        images_of, sorted_unique,
    },
    delivery_cadence_drivers::SamplingNeed,
    podman_container_engine::{
        libpod_running_containers::{EngineContainers, RunningContainer, shared_engine_sampler},
        running_set_changes::deliveries_on_running_set_changes,
    },
    system_protocol_clients::journal_follow::{JournalFollowError, journal_windows},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForgejoTaskOrigin {
    repository: String,
    actions_page: TaskWebLog,
}

fn forgejo_tasks(
    engine: &EngineContainers,
    origins_by_task: &HashMap<u64, ForgejoTaskOrigin>,
    engine_socket: &Path,
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
            repository: origins_by_task
                .get(&task_id)
                .map_or(TaskRepository::NotYetKnown, |origin| {
                    TaskRepository::Named(origin.repository.clone())
                }),
            web_log: origins_by_task
                .get(&task_id)
                .map_or(TaskWebLog::NotKnown, |origin| origin.actions_page.clone()),
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
            usage: TaskUsage::from(containers.as_slice()),
            stop_control: TaskStopControl::of_containers(engine_socket, &containers),
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

impl SnapshotSource for ForgejoRunnerTasks {
    type Error = JournalFollowError;
    type StreamInputs = ForgejoRunnerWatch;

    const NAME: &'static str = "Forgejo runner tasks";

    fn delivery_stream(
        watch: ForgejoRunnerWatch,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let origins_by_task: Arc<Mutex<HashMap<u64, ForgejoTaskOrigin>>> = Arc::default();
        let origins_for_journal = origins_by_task.clone();
        let unit_name = watch.unit_name;
        let task_starts = journal_windows::<ForgejoTaskStart>().filter_map(move |window| {
            let starts = window.unwrap_or_else(|failure| {
                tracing::warn!("{failure}");
                Vec::new()
            });
            if let Ok(mut origins) = origins_for_journal.lock() {
                origins.extend(
                    starts
                        .into_iter()
                        .filter(|start| start.unit_name == unit_name)
                        .map(|start| {
                            (
                                start.task_id,
                                ForgejoTaskOrigin {
                                    repository: start.repository,
                                    actions_page: start.actions_page,
                                },
                            )
                        }),
                );
            }
            ready(None)
        });
        let sampler = shared_engine_sampler(watch.engine_socket.clone());
        let engine_socket = watch.engine_socket;
        let engine_deliveries = deliveries_on_running_set_changes(
            move || {
                let sampler = sampler.clone();
                let origins_by_task = origins_by_task.clone();
                let engine_socket = engine_socket.clone();
                async move {
                    let engine_read = sampler.lock().await.sample().await;
                    let runner = match engine_read {
                        Ok(engine) => {
                            let tasks = origins_by_task.lock().map_or_else(
                                |_| Vec::new(),
                                |origins| forgejo_tasks(&engine, &origins, &engine_socket),
                            );
                            CiRunnerTasks {
                                tasks,
                                access: RunnerAccess::Complete,
                            }
                        }
                        Err(failure) => CiRunnerTasks {
                            tasks: Vec::new(),
                            access: RunnerAccess::EngineUnreadable(failure),
                        },
                    };
                    Ok(ForgejoRunnerTasks { runner })
                }
            },
            |delivered| {
                delivered
                    .as_ref()
                    .map_or(SamplingNeed::KeepSampling, |tasks| {
                        tasks.runner.sampling_need()
                    })
            },
        );
        stream::select(task_starts, engine_deliveries)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use bytesize::ByteSize;
    use uom::si::{f64::Ratio, ratio::percent};

    use super::*;
    use crate::{
        podman_container_engine::libpod_running_containers::ContainerCiIdentity,
        system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
    };

    const TEST_ENGINE_SOCKET: &str = "/run/runner/podman.sock";

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
            cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(1.0)),
            resident_memory: ByteSize::b(memory_bytes),
        }
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
        let origins = HashMap::from([(
            7,
            ForgejoTaskOrigin {
                repository: "wvhulle/bindrune".to_owned(),
                actions_page: TaskWebLog::Linked(
                    "https://codeberg.org/wvhulle/bindrune/actions".to_owned(),
                ),
            },
        )]);
        let tasks = forgejo_tasks(&engine, &origins, Path::new(TEST_ENGINE_SOCKET));
        assert_eq!(tasks.len(), 1);
        assert_eq!(
            tasks[0].repository,
            TaskRepository::Named("wvhulle/bindrune".to_owned())
        );
        assert_eq!(tasks[0].started_at, UNIX_EPOCH + Duration::from_secs(100));
        assert_eq!(tasks[0].activities, ["build"]);
        assert_eq!(
            tasks[0].web_log,
            TaskWebLog::Linked("https://codeberg.org/wvhulle/bindrune/actions".to_owned())
        );
        assert_eq!(
            tasks[0].usage,
            TaskUsage::Measured {
                cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(2.0)),
                resident_memory: ByteSize::b(150),
            }
        );
        assert_eq!(
            tasks[0].stop_control,
            TaskStopControl::StopContainers {
                engine_socket: PathBuf::from(TEST_ENGINE_SOCKET),
                container_ids: vec![
                    "FORGEJO-ACTIONS-TASK-7_WORKFLOW-aa_JOB-build".to_owned(),
                    "FORGEJO-ACTIONS-TASK-7_WORKFLOW-aa_JOB-build_db".to_owned(),
                ],
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
        let tasks = forgejo_tasks(&engine, &HashMap::new(), Path::new(TEST_ENGINE_SOCKET));
        assert_eq!(tasks[0].repository, TaskRepository::NotYetKnown);
        assert_eq!(tasks[0].run_label, "7");
        assert_eq!(tasks[0].web_log, TaskWebLog::NotKnown);
        assert_eq!(tasks[0].activities, ["build"]);
    }
}
