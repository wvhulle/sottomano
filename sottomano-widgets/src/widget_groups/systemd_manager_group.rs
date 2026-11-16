use std::borrow::Cow;

use iced::{Subscription, Task};

use crate::{clock::Clock, widget_groups::widget_group_identity::WidgetGroupFamily};

mod boot_timeline_indicator;
mod failed_unit_log;
mod last_shutdown_indicator;
mod running_job_indicator;

use boot_timeline_indicator::{StartupUnits, boot_timeline_hover_card, boot_timeline_indicator};
use failed_unit_log::{
    failed_units_hover_card, failed_units_item, failed_units_panel_visibility, failed_units_state,
};
use last_shutdown_indicator::{last_shutdown_hover_card, last_shutdown_indicator};
use running_job_indicator::{slow_jobs_hover_card, slow_jobs_item, slow_jobs_panel_visibility};
use sottomano_desktop_sinks::systemd_units_journal::systemd_unit_restarts::restart_failed_unit;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
    systemd_units_journal::{
        SystemdUnitScope,
        boot_timeline::{BootTimeline, BootTimelineWatch, StartupSlowdownThresholds},
        failed_units::{FailedUnitRestart, FailedUnits},
        journal_previous_shutdown::LastShutdown,
        slow_running_jobs::{SlowJobs, SlowJobsWatch},
    },
};
use sottomano_toml_config::{Config, widget_groups::systemd_manager::SystemdManagerConfig};

use crate::{
    background_work::{copy_to_clipboard, logged_failure_task, snapshot_subscription},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        reading_row::PanelVisibility,
    },
    hover_cards::IndicatorKey,
    widget_groups::{
        group_summary::GroupSummary,
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupIdentity,
    },
    widgets::{
        ongoing_toast::OngoingKey,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

const SLOW_JOBS_GLYPH: Phosphor = Phosphor::HourglassMedium(Weight::Regular);
const FAILED_SYSTEM_UNITS_GLYPH: Phosphor = Phosphor::HardDrives(Weight::Regular);
const FAILED_USER_UNITS_GLYPH: Phosphor = Phosphor::User(Weight::Regular);
const LAST_SHUTDOWN_GLYPH: Phosphor = Phosphor::Power(Weight::Regular);

fn unit_scope_icon(scope: SystemdUnitScope) -> Phosphor {
    match scope {
        SystemdUnitScope::UserSession => Phosphor::User(Weight::Regular),
        SystemdUnitScope::SystemWide => Phosphor::HardDrives(Weight::Regular),
    }
}

fn unit_toast_key(row_name: Cow<'static, str>, unit_name: String) -> OngoingKey {
    OngoingKey::SampleDetail {
        indicator: IndicatorKey {
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::SystemdManager),
            name: row_name,
        },
        detail: Cow::Owned(unit_name),
    }
}

fn slow_jobs_entry<'a>(
    snapshot: &'a SubscribedSnapshot<SlowJobs>,
    jobs: &'a SlowJobs,
    config: &'a Config,
    clock: Clock,
) -> GroupIndicator<'a> {
    slow_jobs_item(jobs, config, clock).with_hover_card(Box::new(move |_| {
        snapshot.sections_described_by(slow_jobs_hover_card)
    }))
}

fn failed_units_entry<'a>(
    snapshot: &'a SubscribedSnapshot<FailedUnits>,
    failed: &'a FailedUnits,
    config: &'a Config,
    clock: Clock,
) -> GroupIndicator<'a> {
    failed_units_item(failed, config, clock).with_hover_card(Box::new(move |_| {
        snapshot.sections_described_by(failed_units_hover_card)
    }))
}

#[derive(Default)]
pub struct SystemdManagerGroup {
    boot: SubscribedSnapshot<BootTimeline>,
    failed_system: SubscribedSnapshot<FailedUnits>,
    failed_user: SubscribedSnapshot<FailedUnits>,
    slow_system_jobs: SubscribedSnapshot<SlowJobs>,
    slow_user_jobs: SubscribedSnapshot<SlowJobs>,
    last_shutdown: SubscribedSnapshot<LastShutdown>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum SystemdManagerMessage {
    Boot(SnapshotDelivery<BootTimeline>),
    FailedSystem(SnapshotDelivery<FailedUnits>),
    FailedUser(SnapshotDelivery<FailedUnits>),
    SlowSystemJobs(SnapshotDelivery<SlowJobs>),
    SlowUserJobs(SnapshotDelivery<SlowJobs>),
    LastShutdown(SnapshotDelivery<LastShutdown>),
}

#[derive(Debug, Clone)]
pub enum SystemdManagerInteraction {
    OpenLog(TerminalCommand),
    RestartFailedUnit(FailedUnitRestart),
    InspectCoredump(TerminalCommand),
    CopyToClipboard(String),
    Expansion(WidgetGroupExpansionMessage),
}

impl SystemdManagerGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        config: &SystemdManagerConfig,
        buses: &DesktopBuses,
    ) -> Subscription<SystemdManagerMessage> {
        let systemd = |scope| OnBus {
            buses: buses.clone(),
            inputs: scope,
        };
        Subscription::batch([
            snapshot_subscription::<BootTimeline>(BootTimelineWatch {
                buses: buses.clone(),
                slow_unit: config.slow_boot_unit,
                slowdown_thresholds: StartupSlowdownThresholds {
                    warning_percent: config.slower_boot_warning_percent,
                    critical_percent: config.slower_boot_critical_percent,
                },
            })
            .map(SystemdManagerMessage::Boot),
            snapshot_subscription::<FailedUnits>(systemd(SystemdUnitScope::SystemWide))
                .map(SystemdManagerMessage::FailedSystem),
            snapshot_subscription::<FailedUnits>(systemd(SystemdUnitScope::UserSession))
                .map(SystemdManagerMessage::FailedUser),
            snapshot_subscription::<SlowJobs>(SlowJobsWatch {
                systemd: systemd(SystemdUnitScope::SystemWide),
                slow_after: config.slow_job,
            })
            .map(SystemdManagerMessage::SlowSystemJobs),
            snapshot_subscription::<SlowJobs>(SlowJobsWatch {
                systemd: systemd(SystemdUnitScope::UserSession),
                slow_after: config.slow_job,
            })
            .map(SystemdManagerMessage::SlowUserJobs),
            snapshot_subscription::<LastShutdown>(config.slow_shutdown_stop)
                .map(SystemdManagerMessage::LastShutdown),
        ])
    }

    pub fn update(
        &mut self,
        message: SystemdManagerMessage,
        clock: Clock,
    ) -> Task<SystemdManagerMessage> {
        match message {
            SystemdManagerMessage::Boot(delivery) => {
                self.boot.receive(delivery, clock);
            }
            SystemdManagerMessage::FailedSystem(delivery) => {
                self.failed_system.receive(delivery, clock);
            }
            SystemdManagerMessage::FailedUser(delivery) => {
                self.failed_user.receive(delivery, clock);
            }
            SystemdManagerMessage::SlowSystemJobs(delivery) => {
                self.slow_system_jobs.receive(delivery, clock);
            }
            SystemdManagerMessage::SlowUserJobs(delivery) => {
                self.slow_user_jobs.receive(delivery, clock);
            }
            SystemdManagerMessage::LastShutdown(delivery) => {
                self.last_shutdown.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: SystemdManagerInteraction,
    ) -> Task<SystemdManagerMessage> {
        match interaction {
            SystemdManagerInteraction::OpenLog(journal) => journal.open(),
            SystemdManagerInteraction::RestartFailedUnit(restart) => logged_failure_task(
                format!("systemd: restarting {}", restart.unit),
                restart_failed_unit(restart),
            ),
            SystemdManagerInteraction::InspectCoredump(command) => command.open(),
            SystemdManagerInteraction::CopyToClipboard(text) => copy_to_clipboard(text),
            SystemdManagerInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config, clock: Clock) -> WidgetGroupContent<'a> {
        let locale = config.calendar.locale;
        let mut rows = Vec::new();
        if let Some(timeline) = self.boot.get() {
            rows.push(
                GroupIndicator::sample(boot_timeline_indicator(timeline, locale)).with_hover_card(
                    Box::new(move |_| self.boot.sections_described_by(boot_timeline_hover_card)),
                ),
            );
            rows.push(
                GroupIndicator::sample(StartupUnits { timeline }.reading_row()).with_hover_card(
                    Box::new(move |_| {
                        self.boot
                            .hover_card_with(StartupUnits { timeline }.hover_card())
                    }),
                ),
            );
        }
        for slow_jobs in [&self.slow_system_jobs, &self.slow_user_jobs] {
            match slow_jobs {
                SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                    if slow_jobs_panel_visibility(held_snapshot) == PanelVisibility::Shown {
                        rows.push(slow_jobs_entry(slow_jobs, held_snapshot, config, clock));
                    }
                }
                SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                    let held_snapshot = &troubled_snapshot.held_snapshot;
                    if slow_jobs_panel_visibility(held_snapshot) == PanelVisibility::Shown {
                        rows.push(slow_jobs_entry(slow_jobs, held_snapshot, config, clock));
                    }
                }
                SubscribedSnapshot::NoSnapshot { placeholder } => {
                    rows.push(GroupIndicator::sample(
                        placeholder.reading_row(SLOW_JOBS_GLYPH.into()),
                    ));
                }
                SubscribedSnapshot::AwaitingFirstSnapshot
                | SubscribedSnapshot::SourceUnavailable => {}
            }
        }
        for (failed_units, failed_units_glyph) in [
            (&self.failed_system, FAILED_SYSTEM_UNITS_GLYPH),
            (&self.failed_user, FAILED_USER_UNITS_GLYPH),
        ] {
            match failed_units {
                SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                    if failed_units_panel_visibility(held_snapshot) == PanelVisibility::Shown {
                        rows.push(failed_units_entry(
                            failed_units,
                            held_snapshot,
                            config,
                            clock,
                        ));
                    }
                }
                SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                    let held_snapshot = &troubled_snapshot.held_snapshot;
                    if failed_units_panel_visibility(held_snapshot) == PanelVisibility::Shown {
                        rows.push(failed_units_entry(
                            failed_units,
                            held_snapshot,
                            config,
                            clock,
                        ));
                    }
                }
                SubscribedSnapshot::NoSnapshot { placeholder } => {
                    rows.push(GroupIndicator::sample(
                        placeholder.reading_row(failed_units_glyph.into()),
                    ));
                }
                SubscribedSnapshot::AwaitingFirstSnapshot
                | SubscribedSnapshot::SourceUnavailable => {}
            }
        }
        match &self.last_shutdown {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                rows.push(self.last_shutdown_item(held_snapshot, config));
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                rows.push(self.last_shutdown_item(&troubled_snapshot.held_snapshot, config));
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(GroupIndicator::sample(
                    placeholder.reading_row(LAST_SHUTDOWN_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::SystemdManager,
            GroupSummary::glyph(Phosphor::Speedometer(Weight::Regular).into(), self.state()),
            &self.expansion,
            SystemdManagerInteraction::Expansion,
            rows,
        )
    }

    fn last_shutdown_item<'a>(
        &'a self,
        last_shutdown: &'a LastShutdown,
        config: &'a Config,
    ) -> GroupIndicator<'a> {
        GroupIndicator::sample(last_shutdown_indicator(
            last_shutdown,
            config.calendar.locale,
        ))
        .on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            SystemdManagerInteraction::OpenLog(TerminalCommand::journal(
                config,
                last_shutdown.follow_log_args(),
            )),
        )
        .with_hover_card(Box::new(move |_| {
            self.last_shutdown
                .sections_described_by(last_shutdown_hover_card)
        }))
    }

    fn current_health_states(&self) -> [Option<IndicatorState>; 4] {
        [
            self.slow_system_jobs
                .visible_indicator(slow_jobs_panel_visibility)
                .map(|_| IndicatorState::Warning),
            self.slow_user_jobs
                .visible_indicator(slow_jobs_panel_visibility)
                .map(|_| IndicatorState::Warning),
            self.failed_system
                .visible_indicator(failed_units_panel_visibility)
                .map(|_| failed_units_state()),
            self.failed_user
                .visible_indicator(failed_units_panel_visibility)
                .map(|_| failed_units_state()),
        ]
    }
}

impl SystemdManagerGroup {
    fn state(&self) -> IndicatorState {
        if self.boot.get().is_none() {
            return IndicatorState::Pending;
        }
        self.current_health_states()
            .into_iter()
            .flatten()
            .map(IndicatorState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }
}
