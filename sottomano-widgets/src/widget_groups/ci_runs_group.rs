mod ci_run_readings;
mod ci_runner_watches;

use std::borrow::Cow;

pub use ci_run_readings::CiRunnerKind;
use ci_run_readings::{CiRunnerFailureRow, CiTaskRow, WatchedCiRunner};
pub use ci_runner_watches::{CiRunnerSnapshot, CiRunnerWatches};
use iced::{Subscription, Task};
use sottomano_desktop_sinks::ci_runner_tasks::libpod_container_stop::stop_task_containers;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ci_runner_tasks::{
        TaskStopControl, forgejo_runner_tasks::ForgejoRunnerTasks,
        woodpecker_agent_tasks::WoodpeckerAgentTasks,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        discovered_systemd_services::{DiscoveredSystemdServices, ScopedServiceCommands},
        phosphor::{Phosphor, Weight},
    },
};

const CANCEL_RUN_HINT: &str = "Cancel run";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiRunnerReference {
    kind: CiRunnerKind,
    runner_label: Cow<'static, str>,
}

impl From<&WatchedCiRunner<'_>> for CiRunnerReference {
    fn from(watched: &WatchedCiRunner<'_>) -> Self {
        Self {
            kind: watched.kind,
            runner_label: Cow::Owned(watched.runner_label.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiRunsTarget {
    Runner(CiRunnerReference),
    Run {
        runner: CiRunnerReference,
        run_label: Cow<'static, str>,
    },
}

impl CiRunsTarget {
    #[must_use]
    pub const fn run(
        kind: CiRunnerKind,
        runner_label: &'static str,
        run_label: &'static str,
    ) -> Self {
        CiRunsTarget::Run {
            runner: CiRunnerReference {
                kind,
                runner_label: Cow::Borrowed(runner_label),
            },
            run_label: Cow::Borrowed(run_label),
        }
    }
}

impl From<CiRunsTarget> for HoverTarget {
    fn from(target: CiRunsTarget) -> Self {
        HoverTarget::CiRuns(target)
    }
}

#[derive(Default)]
pub struct CiRunsGroup {
    services: DiscoveredSystemdServices,
    woodpecker_watches: CiRunnerWatches<WoodpeckerAgentTasks>,
    forgejo_watches: CiRunnerWatches<ForgejoRunnerTasks>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CiRunsMessage {
    Services(ScopedServiceCommands),
    WoodpeckerRunner(String, SnapshotDelivery<WoodpeckerAgentTasks>),
    ForgejoRunner(String, SnapshotDelivery<ForgejoRunnerTasks>),
    Expansion(WidgetGroupExpansionMessage),
    CancelRun(String, TaskStopControl),
    CancelAccepted(String),
    CancelRefused(String, String),
}

fn cancel_run_task(run_label: String, stop_control: TaskStopControl) -> Task<CiRunsMessage> {
    match stop_control {
        TaskStopControl::NoContainerNow => Task::none(),
        TaskStopControl::StopContainers {
            engine_socket,
            container_ids,
        } => Task::perform(
            async move { stop_task_containers(&engine_socket, &container_ids).await },
            move |stopped| match stopped {
                Ok(()) => CiRunsMessage::CancelAccepted(run_label),
                Err(failure) => CiRunsMessage::CancelRefused(run_label, failure),
            },
        ),
    }
}

fn runner_hover_card<S: CiRunnerSnapshot>(
    watches: &CiRunnerWatches<S>,
    runner_label: &str,
    describe: impl FnOnce(WatchedCiRunner<'_>) -> Option<HoverCardBody>,
) -> Option<HoverCard> {
    let snapshot = watches.snapshot_of(runner_label)?;
    let body = describe(WatchedCiRunner {
        kind: S::KIND,
        runner_label,
        runner: snapshot.get()?.runner(),
    })?;
    snapshot.hover_card_with(body)
}

fn watched_runners_of<S: CiRunnerSnapshot>(
    watches: &CiRunnerWatches<S>,
) -> impl Iterator<Item = WatchedCiRunner<'_>> {
    watches
        .delivered_runners()
        .map(|(runner_label, runner)| WatchedCiRunner {
            kind: S::KIND,
            runner_label,
            runner,
        })
}

impl CiRunsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<CiRunsMessage> {
        Subscription::batch([
            DiscoveredSystemdServices::subscription(buses).map(CiRunsMessage::Services),
            self.woodpecker_watches
                .subscription()
                .map(|(runner_label, delivery)| {
                    CiRunsMessage::WoodpeckerRunner(runner_label, delivery)
                }),
            self.forgejo_watches
                .subscription()
                .map(|(runner_label, delivery)| {
                    CiRunsMessage::ForgejoRunner(runner_label, delivery)
                }),
        ])
    }

    pub fn update(&mut self, message: CiRunsMessage, clock: Clock) -> Task<CiRunsMessage> {
        match message {
            CiRunsMessage::Services(scoped_commands) => {
                self.services.receive(scoped_commands, clock);
                self.woodpecker_watches.adopt_discovered(&self.services);
                self.forgejo_watches.adopt_discovered(&self.services);
            }
            CiRunsMessage::WoodpeckerRunner(runner_label, delivery) => {
                self.woodpecker_watches
                    .receive(runner_label, delivery, clock);
            }
            CiRunsMessage::ForgejoRunner(runner_label, delivery) => {
                self.forgejo_watches.receive(runner_label, delivery, clock);
            }
            CiRunsMessage::Expansion(message) => {
                self.expansion.update(message);
            }
            CiRunsMessage::CancelRun(run_label, stop_control) => {
                return cancel_run_task(run_label, stop_control);
            }
            CiRunsMessage::CancelAccepted(run_label) => {
                tracing::info!("stopped the containers of CI run {run_label}");
            }
            CiRunsMessage::CancelRefused(run_label, failure) => {
                tracing::warn!("cancelling CI run {run_label}: {failure}");
            }
        }
        Task::none()
    }

    fn watched_runners(&self) -> Vec<WatchedCiRunner<'_>> {
        watched_runners_of(&self.woodpecker_watches)
            .chain(watched_runners_of(&self.forgejo_watches))
            .collect()
    }

    fn runner_hover_card(
        &self,
        reference: &CiRunnerReference,
        describe: impl FnOnce(WatchedCiRunner<'_>) -> Option<HoverCardBody>,
    ) -> Option<HoverCard> {
        match reference.kind {
            CiRunnerKind::Woodpecker => {
                runner_hover_card(&self.woodpecker_watches, &reference.runner_label, describe)
            }
            CiRunnerKind::Forgejo => {
                runner_hover_card(&self.forgejo_watches, &reference.runner_label, describe)
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &CiRunsTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            CiRunsTarget::Runner(reference) => self.runner_hover_card(reference, |watched| {
                CiRunnerFailureRow::of_failing(watched).map(|row| row.hover_card(clock))
            }),
            CiRunsTarget::Run { runner, run_label } => self.runner_hover_card(runner, |watched| {
                let task = watched
                    .runner
                    .tasks
                    .iter()
                    .find(|task| task.run_label == *run_label)?;
                Some(
                    CiTaskRow {
                        kind: watched.kind,
                        task,
                    }
                    .hover_card(clock),
                )
            }),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, CiRunsMessage> {
        let watched_runners = self.watched_runners();
        let total_runs = watched_runners
            .iter()
            .map(|watched| watched.runner.tasks.len())
            .sum();
        let runner_rows = watched_runners.into_iter().flat_map(|watched| {
            let reference = CiRunnerReference::from(&watched);
            let failure_row = CiRunnerFailureRow::of_failing(watched)
                .map(|row| GroupItem::reading(row, CiRunsTarget::Runner(reference.clone())));
            let run_rows = watched.runner.tasks.iter().map(move |task| {
                let row = GroupItem::reading(
                    CiTaskRow {
                        kind: watched.kind,
                        task,
                    },
                    CiRunsTarget::Run {
                        runner: reference.clone(),
                        run_label: Cow::Owned(task.run_label.clone()),
                    },
                );
                match &task.stop_control {
                    TaskStopControl::NoContainerNow => row,
                    stop_control @ TaskStopControl::StopContainers { .. } => row.on_right_press(
                        CANCEL_RUN_HINT,
                        CiRunsMessage::CancelRun(task.run_label.clone(), stop_control.clone()),
                    ),
                }
            });
            failure_row.into_iter().chain(run_rows)
        });
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::sole(WidgetGroupFamily::CiRuns),
            WidgetGroupTitle::Named(ci_runs_title(total_runs).into()),
            &self.expansion,
            CiRunsMessage::Expansion,
            runner_rows,
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::FlowArrow(Weight::Regular).into(),
        })
    }
}

fn ci_runs_title(total_runs: usize) -> String {
    match total_runs {
        1 => "1 CI run".to_owned(),
        _ => format!("{total_runs} CI runs"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_counts_the_active_runs() {
        assert_eq!(ci_runs_title(0), "0 CI runs");
        assert_eq!(ci_runs_title(1), "1 CI run");
        assert_eq!(ci_runs_title(3), "3 CI runs");
    }
}
