use std::borrow::Cow;

use iced::{Subscription, Task};

use crate::{clock::Clock, widget_groups::widget_group_identity::WidgetGroupFamily};

mod boot_timeline_reading;
mod failed_unit_log;
mod last_shutdown_reading;
mod running_job_reading;

use boot_timeline_reading::{StartupUnits, boot_timeline_hover_card, boot_timeline_reading_row};
use failed_unit_log::{
    failed_units_hover_card, failed_units_item, failed_units_panel_visibility,
    failed_units_reading_row, failed_units_state,
};
use last_shutdown_reading::{last_shutdown_hover_card, last_shutdown_reading_row};
use running_job_reading::{
    slow_jobs_hover_card, slow_jobs_item, slow_jobs_panel_visibility, slow_jobs_reading_row,
};
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
    group_items::{group_item::GroupItem, reading_display_state::ReadingDisplayState},
    hover_cards::InstrumentKey,
    widget_groups::{
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupIdentity,
    },
    widgets::{
        icon_glyph::IconGlyph,
        ongoing_toast::OngoingKey,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

fn unit_scope_icon(scope: SystemdUnitScope) -> Phosphor {
    match scope {
        SystemdUnitScope::UserSession => Phosphor::User(Weight::Regular),
        SystemdUnitScope::SystemWide => Phosphor::HardDrives(Weight::Regular),
    }
}

fn unit_toast_key(row_name: Cow<'static, str>, unit_name: String) -> OngoingKey {
    OngoingKey::ReadingDetail {
        instrument: InstrumentKey {
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::SystemdManager),
            name: row_name,
        },
        detail: Cow::Owned(unit_name),
    }
}

fn slow_jobs_entry<'a>(
    snapshot: &'a SubscribedSnapshot<SlowJobs>,
    config: &'a Config,
    clock: Clock,
) -> Option<GroupItem<'a, SystemdManagerInteraction>> {
    snapshot
        .visible_reading(slow_jobs_panel_visibility)
        .map(|jobs| {
            slow_jobs_item(jobs, config, clock).with_hover_card(Box::new(move |_| {
                snapshot.sections_described_by(slow_jobs_hover_card)
            }))
        })
        .or_else(|| {
            snapshot.group_item(
                IconGlyph::default(),
                |slow_jobs| slow_jobs_reading_row(slow_jobs),
                clock,
            )
        })
}

fn failed_units_entry<'a>(
    snapshot: &'a SubscribedSnapshot<FailedUnits>,
    config: &'a Config,
    clock: Clock,
) -> Option<GroupItem<'a, SystemdManagerInteraction>> {
    snapshot
        .visible_reading(failed_units_panel_visibility)
        .map(|failed| {
            failed_units_item(failed, config, clock).with_hover_card(Box::new(move |_| {
                snapshot.sections_described_by(failed_units_hover_card)
            }))
        })
        .or_else(|| {
            snapshot.group_item(
                IconGlyph::default(),
                |failed_units| failed_units_reading_row(failed_units),
                clock,
            )
        })
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
    Interaction(SystemdManagerInteraction),
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
            SystemdManagerMessage::Interaction(SystemdManagerInteraction::OpenLog(journal)) => {
                return journal.open();
            }
            SystemdManagerMessage::Interaction(SystemdManagerInteraction::RestartFailedUnit(
                restart,
            )) => {
                return logged_failure_task(
                    format!("systemd: restarting {}", restart.unit),
                    restart_failed_unit(restart),
                );
            }
            SystemdManagerMessage::Interaction(SystemdManagerInteraction::InspectCoredump(
                command,
            )) => return command.open(),
            SystemdManagerMessage::Interaction(SystemdManagerInteraction::CopyToClipboard(
                text,
            )) => return copy_to_clipboard(text),
            SystemdManagerMessage::Interaction(SystemdManagerInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, SystemdManagerInteraction> {
        let locale = config.calendar.locale;
        let boot = self.boot.get();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::SystemdManager,
            &self.expansion,
            SystemdManagerInteraction::Expansion,
            [
                boot.map(|boot| {
                    GroupItem::reading(boot_timeline_reading_row(boot, locale))
                        .previewed_on_handle()
                        .with_hover_card(Box::new(move |_| {
                            self.boot.sections_described_by(boot_timeline_hover_card)
                        }))
                }),
                boot.map(|timeline| {
                    GroupItem::reading(StartupUnits { timeline }.reading_row()).with_hover_card(
                        Box::new(move |_| {
                            self.boot
                                .hover_card_with(StartupUnits { timeline }.hover_card())
                        }),
                    )
                }),
                slow_jobs_entry(&self.slow_system_jobs, config, clock),
                slow_jobs_entry(&self.slow_user_jobs, config, clock),
                failed_units_entry(&self.failed_system, config, clock),
                failed_units_entry(&self.failed_user, config, clock),
                self.last_shutdown
                    .get()
                    .map(|last| {
                        GroupItem::reading(last_shutdown_reading_row(last, locale))
                            .on_press(
                                TerminalCommand::JOURNAL_PRESS_HINT,
                                SystemdManagerInteraction::OpenLog(TerminalCommand::journal(
                                    config,
                                    last.follow_log_args(),
                                )),
                            )
                            .with_hover_card(Box::new(move |_| {
                                self.last_shutdown
                                    .sections_described_by(last_shutdown_hover_card)
                            }))
                    })
                    .or_else(|| {
                        self.last_shutdown.group_item(
                            IconGlyph::default(),
                            |snapshot| last_shutdown_reading_row(snapshot, locale),
                            clock,
                        )
                    }),
            ]
            .into_iter()
            .flatten(),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::Speedometer(Weight::Regular).into(),
            state: self.state(),
        })
    }

    fn current_health_states(&self) -> [Option<ReadingDisplayState>; 4] {
        [
            self.slow_system_jobs
                .visible_reading(slow_jobs_panel_visibility)
                .map(|_| ReadingDisplayState::Warning),
            self.slow_user_jobs
                .visible_reading(slow_jobs_panel_visibility)
                .map(|_| ReadingDisplayState::Warning),
            self.failed_system
                .visible_reading(failed_units_panel_visibility)
                .map(|_| failed_units_state()),
            self.failed_user
                .visible_reading(failed_units_panel_visibility)
                .map(|_| failed_units_state()),
        ]
    }
}

impl SystemdManagerGroup {
    fn state(&self) -> ReadingDisplayState {
        if self.boot.get().is_none() {
            return ReadingDisplayState::Pending;
        }
        self.current_health_states()
            .into_iter()
            .flatten()
            .map(ReadingDisplayState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }
}
