use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskStopControl,
        TaskUsage, TaskWebLog, forgejo_runner_tasks::ForgejoRunnerTasks,
        woodpecker_agent_tasks::WoodpeckerAgentTasks,
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
    systemd_units_journal::{SystemdUnitScope, service_commands::ServiceAccount},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::ci_runs_group::{CiRunsGroup, CiRunsMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    fixtures::systemd_service_units::{ShownService, services_delivered},
    showcased_group::ShowcasedGroup,
};

pub const SHOWN_RUNNER: &str = "codeberg";
pub const HOVERED_RUN: &str = "#185";

fn runner_service<'a>(
    unit: &'a str,
    program_name: &'a str,
    arguments: &'a [&'a str],
    docker_host: &'a [(&'a str, &'a str)],
) -> ShownService<'a> {
    ShownService {
        unit,
        program_name,
        arguments,
        environment: docker_host,
        account: ServiceAccount::NamedUser(unit.trim_end_matches(".service").to_owned()),
    }
}

fn polling_cadence() -> DeliveryCadence {
    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5)))
}

fn nix_task(
    repository: &str,
    run_label: &str,
    activity: &str,
    usage: TaskUsage,
    running_for: Duration,
    clock: Clock,
) -> CiTask {
    CiTask {
        repository: TaskRepository::Named(repository.to_owned()),
        run_label: run_label.to_owned(),
        started_at: clock.wall - running_for,
        deadline: TaskDeadline::NotExposed,
        activities: vec![activity.to_owned()],
        images: vec!["docker.io/nixos/nix:latest".to_owned()],
        usage,
        web_log: TaskWebLog::NotKnown,
        stop_control: TaskStopControl::NoContainerNow,
    }
}

fn woodpecker_delivery(clock: Clock) -> SnapshotDelivery<WoodpeckerAgentTasks> {
    let build_site = nix_task(
        "demo/website",
        HOVERED_RUN,
        "run-tests",
        TaskUsage::Measured {
            cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(18.6)),
            resident_memory: ByteSize::gib(4),
        },
        Duration::from_mins(10),
        clock,
    );
    let lint_library = nix_task(
        "demo/library",
        "#201",
        "clone",
        TaskUsage::Measured {
            cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(11.4)),
            resident_memory: ByteSize::gib(1),
        },
        Duration::from_mins(2),
        clock,
    );
    SnapshotDelivery::fixture_delivered(
        Ok(WoodpeckerAgentTasks {
            runner: CiRunnerTasks {
                tasks: vec![
                    CiTask {
                        deadline: TaskDeadline::At(clock.wall + Duration::from_mins(50)),
                        web_log: TaskWebLog::Linked(
                            "https://ci.example.org/repos/12/pipeline/185".to_owned(),
                        ),
                        ..build_site
                    },
                    CiTask {
                        deadline: TaskDeadline::At(clock.wall + Duration::from_mins(58)),
                        ..lint_library
                    },
                ],
                access: RunnerAccess::Complete,
            },
        }),
        polling_cadence(),
    )
}

fn forgejo_delivery(clock: Clock) -> SnapshotDelivery<ForgejoRunnerTasks> {
    SnapshotDelivery::fixture_delivered(
        Ok(ForgejoRunnerTasks {
            runner: CiRunnerTasks {
                tasks: vec![CiTask {
                    web_log: TaskWebLog::Linked(
                        "https://forge.example.org/demo/tool/actions".to_owned(),
                    ),
                    ..nix_task(
                        "demo/tool",
                        "11282424",
                        "build",
                        TaskUsage::Measured {
                            cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(3.3)),
                            resident_memory: ByteSize::mib(640),
                        },
                        Duration::from_mins(1),
                        clock,
                    )
                }],
                access: RunnerAccess::Complete,
            },
        }),
        polling_cadence(),
    )
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = CiRunsGroup::default();
    let _ = group.update(
        CiRunsMessage::Services(services_delivered(
            SystemdUnitScope::SystemWide,
            [
                runner_service(
                    "woodpecker-agent-codeberg.service",
                    "woodpecker-agent",
                    &[],
                    &[(
                        "DOCKER_HOST",
                        "unix:///run/woodpecker-agent-codeberg/podman.sock",
                    )],
                ),
                runner_service(
                    "forgejo-runner-codeberg.service",
                    "forgejo-runner",
                    &["daemon", "--config", "/etc/forgejo-runner/codeberg.yaml"],
                    &[(
                        "DOCKER_HOST",
                        "unix:///run/forgejo-runner-codeberg/podman.sock",
                    )],
                ),
            ],
        )),
        clock,
    );
    let _ = group.update(
        CiRunsMessage::WoodpeckerRunner(SHOWN_RUNNER.to_owned(), woodpecker_delivery(clock)),
        clock,
    );
    let _ = group.update(
        CiRunsMessage::ForgejoRunner(SHOWN_RUNNER.to_owned(), forgejo_delivery(clock)),
        clock,
    );
    ShowcasedGroup::CiRuns(Box::new(group))
}
