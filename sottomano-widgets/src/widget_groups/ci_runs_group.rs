use chrono::Locale;

use crate::group_indicators::indicator_state::IndicatorState;
mod ci_run_indicators;
mod ci_runner_watches;

use std::borrow::Cow;

pub use ci_run_indicators::CiRunnerKind;
use ci_run_indicators::{CiRunnerFailureIndicator, CiTaskIndicator, WatchedCiRunner};
pub use ci_runner_watches::{CiRunnerSnapshot, CiRunnerWatches};
use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    ci_runner_tasks::libpod_container_stop::stop_task_containers,
    sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ci_runner_tasks::{
        CiTask, TaskStopControl, forgejo_runner_tasks::ForgejoRunnerTasks,
        woodpecker_agent_tasks::WoodpeckerAgentTasks,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    clock::Clock,
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
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

#[derive(Default)]
pub struct CiRunsGroup {
    services: DiscoveredSystemdServices,
    woodpecker_watches: CiRunnerWatches<WoodpeckerAgentTasks>,
    forgejo_watches: CiRunnerWatches<ForgejoRunnerTasks>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum CiRunsMessage {
    Services(ScopedServiceCommands),
    WoodpeckerRunner(String, SnapshotDelivery<WoodpeckerAgentTasks>),
    ForgejoRunner(String, SnapshotDelivery<ForgejoRunnerTasks>),
    CancelAccepted(String),
    CancelRefused(String, GuestPowerError),
}

#[derive(Debug, Clone)]
pub enum CiRunsInteraction {
    Expansion(WidgetGroupExpansionMessage),
    CancelRun(String, TaskStopControl),
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
    describe: impl FnOnce(WatchedCiRunner<'_>) -> Option<Vec<Section>>,
) -> Vec<Section> {
    let Some(snapshot) = watches.snapshot_of(runner_label) else {
        return Vec::new();
    };
    let Some(held_snapshot) = snapshot.get() else {
        return Vec::new();
    };
    let Some(body) = describe(WatchedCiRunner {
        kind: S::KIND,
        runner_label,
        runner: held_snapshot.runner(),
    }) else {
        return Vec::new();
    };
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
            CiRunsMessage::CancelAccepted(run_label) => {
                tracing::info!("stopped the containers of CI run {run_label}");
            }
            CiRunsMessage::CancelRefused(run_label, failure) => {
                tracing::warn!("cancelling CI run {run_label}: {failure}");
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: CiRunsInteraction) -> Task<CiRunsMessage> {
        match interaction {
            CiRunsInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            CiRunsInteraction::CancelRun(run_label, stop_control) => {
                cancel_run_task(run_label, stop_control)
            }
        }
    }

    fn watched_runners(&self) -> Vec<WatchedCiRunner<'_>> {
        watched_runners_of(&self.woodpecker_watches)
            .chain(watched_runners_of(&self.forgejo_watches))
            .collect()
    }

    fn runner_hover_card(
        &self,
        reference: &CiRunnerReference,
        describe: impl FnOnce(WatchedCiRunner<'_>) -> Option<Vec<Section>>,
    ) -> Vec<Section> {
        match reference.kind {
            CiRunnerKind::Woodpecker => {
                runner_hover_card(&self.woodpecker_watches, &reference.runner_label, describe)
            }
            CiRunnerKind::Forgejo => {
                runner_hover_card(&self.forgejo_watches, &reference.runner_label, describe)
            }
        }
    }

    pub fn group_content(&self, clock: Clock, locale: Locale) -> WidgetGroupContent<'_> {
        let watched_runners = self.watched_runners();
        let total_runs = watched_runners
            .iter()
            .map(|watched| watched.runner.tasks.len())
            .sum();
        let mut rows = Vec::new();
        for watched in watched_runners {
            let reference = CiRunnerReference::from(&watched);
            if let Some(failure_indicator) = CiRunnerFailureIndicator::failing_runner(watched) {
                rows.push(self.runner_failure_indicator(failure_indicator, reference.clone()));
            }
            rows.extend(watched.runner.tasks.iter().map(|task| {
                self.ci_task_indicator(watched.kind, task, reference.clone(), clock, locale)
            }));
        }
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::sole(WidgetGroupFamily::CiRuns),
            WidgetGroupTitle::Named(ci_runs_title(total_runs).into()),
            GroupIndicator::glyph_only(
                WidgetGroupTitle::Named(ci_runs_title(total_runs).into()).to_string(),
                Phosphor::FlowArrow(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            CiRunsInteraction::Expansion,
            rows,
        )
    }

    fn runner_failure_indicator<'a>(
        &'a self,
        failure_indicator: CiRunnerFailureIndicator<'a>,
        failure_reference: CiRunnerReference,
    ) -> GroupIndicator<'a> {
        failure_indicator
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.runner_hover_card(&failure_reference, |watched_runner| {
                    CiRunnerFailureIndicator::failing_runner(watched_runner)
                        .map(|failing_indicator| failing_indicator.hover_card())
                })
            }))
    }

    fn ci_task_indicator<'a>(
        &'a self,
        runner_kind: CiRunnerKind,
        task: &'a CiTask,
        task_reference: CiRunnerReference,
        clock: Clock,
        locale: Locale,
    ) -> GroupIndicator<'a> {
        let task_indicator = CiTaskIndicator {
            kind: runner_kind,
            task,
        }
        .indicator(clock, locale)
        .with_hover_card(Box::new(move |_| {
            self.runner_hover_card(&task_reference, |watched_runner| {
                Some(
                    CiTaskIndicator {
                        kind: watched_runner.kind,
                        task,
                    }
                    .hover_card(),
                )
            })
        }));
        match &task.stop_control {
            TaskStopControl::NoContainerNow => task_indicator,
            stop_control @ TaskStopControl::StopContainers { .. } => task_indicator.on_right_press(
                CANCEL_RUN_HINT,
                CiRunsInteraction::CancelRun(task.run_label.clone(), stop_control.clone()),
            ),
        }
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
