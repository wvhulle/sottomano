use std::{collections::BTreeMap, convert::Infallible, path::PathBuf, time::SystemTime};

use futures::{Stream, StreamExt};

use crate::{
    SnapshotDelivery, SnapshotSource,
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskUsage, images_of,
        sorted_unique,
        woodpecker_agent_varz::{AgentVarz, RunningWorkflow, fetch_agent_varz},
    },
    podman_container_engine::{
        libpod_running_containers::{
            ContainerCiIdentity, EngineContainers, EngineReadError, RunningContainer,
            shared_engine_sampler,
        },
        running_set_changes::samples_on_running_set_changes,
    },
    stream_operators::StreamOperators,
    system_protocol_clients::http_json_client,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PipelineKey {
    repository: String,
    pipeline_number: String,
}

fn containers_of_pipeline<'engine>(
    engine: &'engine EngineContainers,
    key: &PipelineKey,
) -> Vec<&'engine RunningContainer> {
    engine
        .containers
        .iter()
        .filter(|container| match &container.ci_identity {
            ContainerCiIdentity::WoodpeckerStep(identity) => {
                identity.repository == key.repository
                    && identity.pipeline_number == key.pipeline_number
            }
            ContainerCiIdentity::NotWoodpecker => false,
        })
        .collect()
}

fn woodpecker_tasks(varz: &AgentVarz, engine: &EngineContainers) -> Vec<CiTask> {
    let workflows_by_pipeline = varz.running.values().fold(
        BTreeMap::<PipelineKey, Vec<&RunningWorkflow>>::new(),
        |mut workflows_by_pipeline, workflow| {
            workflows_by_pipeline
                .entry(PipelineKey {
                    repository: workflow.repository.clone(),
                    pipeline_number: workflow.pipeline_number.clone(),
                })
                .or_default()
                .push(workflow);
            workflows_by_pipeline
        },
    );
    workflows_by_pipeline
        .into_iter()
        .map(|(key, workflows)| {
            let started_at = workflows
                .iter()
                .map(|workflow| workflow.started_at())
                .min()
                .unwrap_or_else(SystemTime::now);
            let deadline = workflows
                .iter()
                .map(|workflow| workflow.deadline())
                .min()
                .map_or(TaskDeadline::NotExposed, TaskDeadline::At);
            let containers = containers_of_pipeline(engine, &key);
            CiTask {
                run_label: format!("#{}", key.pipeline_number),
                repository: TaskRepository::Named(key.repository),
                started_at,
                deadline,
                activities: sorted_unique(
                    containers
                        .iter()
                        .map(|container| container.shown_name.clone()),
                ),
                images: images_of(&containers),
                usage: TaskUsage::of_containers(&containers),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct WoodpeckerAgentTasks {
    pub runner: CiRunnerTasks,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WoodpeckerAgentWatch {
    pub varz_url: String,
    pub engine_socket: PathBuf,
}

fn joined_agent_tasks(
    varz: &AgentVarz,
    engine_read: Result<EngineContainers, EngineReadError>,
) -> CiRunnerTasks {
    match engine_read {
        Ok(engine) => CiRunnerTasks {
            tasks: woodpecker_tasks(varz, &engine),
            access: RunnerAccess::Complete,
        },
        Err(failure) => CiRunnerTasks {
            tasks: woodpecker_tasks(
                varz,
                &EngineContainers {
                    containers: Vec::new(),
                },
            ),
            access: RunnerAccess::EngineUnreadable(failure),
        },
    }
}

fn unreachable_agent(failure: http_json_client::Error) -> CiRunnerTasks {
    CiRunnerTasks {
        tasks: Vec::new(),
        access: RunnerAccess::AgentUnreachable(failure),
    }
}

impl SnapshotSource for WoodpeckerAgentTasks {
    type Error = Infallible;
    type StreamInputs = WoodpeckerAgentWatch;

    const NAME: &'static str = "Woodpecker agent tasks";

    fn delivery_stream(
        watch: WoodpeckerAgentWatch,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let sampler = shared_engine_sampler(watch.engine_socket);
        let varz_url = watch.varz_url;
        samples_on_running_set_changes(
            move || {
                let sampler = sampler.clone();
                let varz_url = varz_url.clone();
                async move {
                    let engine_read = sampler.lock().await.sample().await;
                    fetch_agent_varz(&varz_url)
                        .await
                        .map_or_else(unreachable_agent, |varz| {
                            joined_agent_tasks(&varz, engine_read)
                        })
                }
            },
            CiRunnerTasks::keeps_sampling,
        )
        .map(|tracked| {
            tracked.stamped_cadence.delivered(Ok(WoodpeckerAgentTasks {
                runner: tracked.sample,
            }))
        })
        .repeats_as_unchanged()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::UNIX_EPOCH};

    use bytesize::ByteSize;
    use chrono::DateTime;
    use serde_json::json;

    use super::*;
    use crate::{
        ci_runner_tasks::woodpecker_agent_varz::captured_varz,
        podman_container_engine::libpod_inspect_ci_identity::WoodpeckerStepIdentity,
        system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
    };

    fn step_container(
        repository: &str,
        pipeline_number: &str,
        step_name: &str,
    ) -> RunningContainer {
        RunningContainer {
            id: step_name.to_owned(),
            container_name: format!("wp_{step_name}"),
            shown_name: step_name.to_owned(),
            image: "docker.io/nixos/nix:latest".to_owned(),
            started_at: UNIX_EPOCH,
            ci_identity: ContainerCiIdentity::WoodpeckerStep(WoodpeckerStepIdentity {
                repository: repository.to_owned(),
                pipeline_number: pipeline_number.to_owned(),
                workflow_name: "checks".to_owned(),
                step_name: step_name.to_owned(),
            }),
            cpu_load: ProcessTreeCpuLoad::BusyCores(1.5),
            resident_memory: ByteSize::b(2048),
        }
    }

    #[test]
    fn usage_is_attributed_to_the_pipeline_that_owns_the_container() {
        let engine = EngineContainers {
            containers: vec![
                step_container("wvhulle/sottomano", "185", "test"),
                step_container("wvhulle/sottomano", "185", "clippy"),
                step_container("wvhulle/nu-lint", "201", "clone"),
            ],
        };
        let tasks = woodpecker_tasks(&captured_varz(), &engine);
        assert_eq!(tasks.len(), 2);
        let sottomano = tasks
            .iter()
            .find(|task| task.run_label == "#185")
            .expect("sottomano task is listed");
        assert_eq!(sottomano.activities, ["clippy", "test"]);
        assert_eq!(
            sottomano.usage,
            TaskUsage::Measured {
                cpu_load: ProcessTreeCpuLoad::BusyCores(3.0),
                resident_memory: ByteSize::b(4096),
            }
        );
    }

    #[test]
    fn a_task_between_steps_stays_listed_without_usage() {
        let tasks = woodpecker_tasks(
            &captured_varz(),
            &EngineContainers {
                containers: Vec::new(),
            },
        );
        assert_eq!(tasks.len(), 2);
        assert!(
            tasks
                .iter()
                .all(|task| task.usage == TaskUsage::NoContainerNow)
        );
    }

    #[test]
    fn parallel_workflows_of_one_pipeline_share_one_task() {
        let varz: AgentVarz = serde_json::from_value(json!({
            "running": {
                "1": {
                    "repository": "wvhulle/sottomano",
                    "pipeline_number": "185",
                    "pipeline_started": "2026-11-05T17:43:26Z",
                    "pipeline_timeout": 1_000_000_000_u64,
                },
                "2": {
                    "repository": "wvhulle/sottomano",
                    "pipeline_number": "185",
                    "pipeline_started": "2026-11-05T17:40:00Z",
                    "pipeline_timeout": 1_000_000_000_u64,
                },
            },
        }))
        .expect("varz decodes");
        let tasks = woodpecker_tasks(
            &varz,
            &EngineContainers {
                containers: Vec::new(),
            },
        );
        assert_eq!(tasks.len(), 1);
        assert_eq!(
            tasks[0].started_at,
            SystemTime::from(
                DateTime::parse_from_rfc3339("2026-11-05T17:40:00Z").expect("timestamp parses")
            )
        );
    }

    #[test]
    fn a_pipeline_between_steps_keeps_usage_sampling_and_an_idle_agent_stops_it() {
        let empty_engine = || {
            Ok(EngineContainers {
                containers: Vec::new(),
            })
        };
        assert!(joined_agent_tasks(&captured_varz(), empty_engine()).keeps_sampling());
        let idle_varz = AgentVarz {
            running: HashMap::new(),
        };
        assert!(!joined_agent_tasks(&idle_varz, empty_engine()).keeps_sampling());
        assert!(
            joined_agent_tasks(&idle_varz, Err(EngineReadError::SocketDenied)).keeps_sampling()
        );
    }

    #[test]
    fn a_denied_engine_keeps_the_task_list() {
        let joined = joined_agent_tasks(&captured_varz(), Err(EngineReadError::SocketDenied));
        assert_eq!(joined.tasks.len(), 2);
        assert_eq!(
            joined.access,
            RunnerAccess::EngineUnreadable(EngineReadError::SocketDenied)
        );
    }
}
