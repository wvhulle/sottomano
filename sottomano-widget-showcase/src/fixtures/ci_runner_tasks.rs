use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskUsage,
        forgejo_runner_journal_repos::ForgejoRunnerTasks,
        woodpecker_agent_varz::WoodpeckerAgentTasks,
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    podman_container_engine::libpod_running_containers::ContainerCpuLoad,
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::{
        ci_task_group_item::CiTasksMessage, forgejo_tasks_group::ForgejoTasksGroup,
        woodpecker_tasks_group::WoodpeckerTasksGroup,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub const SHOWN_RUNNER: &str = "codeberg";

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
    }
}

pub fn woodpecker_group(clock: Clock) -> ShowcasedGroup {
    let mut group = WoodpeckerTasksGroup::default();
    let build_site = nix_task(
        "demo/website",
        "#185",
        "run-tests",
        TaskUsage::Measured {
            cpu_load: ContainerCpuLoad::BusyCores(6.2),
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
            cpu_load: ContainerCpuLoad::BusyCores(3.8),
            resident_memory: ByteSize::gib(1),
        },
        Duration::from_mins(2),
        clock,
    );
    group.update(
        CiTasksMessage::Runner(
            SHOWN_RUNNER.to_owned(),
            SnapshotDelivery::fixture_delivered(
                Ok(WoodpeckerAgentTasks {
                    runner: CiRunnerTasks {
                        tasks: vec![
                            CiTask {
                                deadline: TaskDeadline::At(clock.wall + Duration::from_mins(50)),
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
            ),
        ),
        clock,
    );
    ShowcasedGroup::WoodpeckerTasks(Box::new(group))
}

pub fn forgejo_group(clock: Clock) -> ShowcasedGroup {
    let mut group = ForgejoTasksGroup::default();
    group.update(
        CiTasksMessage::Runner(
            SHOWN_RUNNER.to_owned(),
            SnapshotDelivery::fixture_delivered(
                Ok(ForgejoRunnerTasks {
                    runner: CiRunnerTasks {
                        tasks: vec![nix_task(
                            "demo/tool",
                            "11282424",
                            "build",
                            TaskUsage::Measured {
                                cpu_load: ContainerCpuLoad::BusyCores(1.1),
                                resident_memory: ByteSize::mib(640),
                            },
                            Duration::from_mins(1),
                            clock,
                        )],
                        access: RunnerAccess::Complete,
                    },
                }),
                polling_cadence(),
            ),
        ),
        clock,
    );
    ShowcasedGroup::ForgejoTasks(Box::new(group))
}
