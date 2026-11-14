use std::borrow::Cow;

use iced::{Subscription, Task};

use crate::{clock::Clock, widget_groups::widget_group_identity::WidgetGroupFamily};

mod boot_timeline_reading;
mod failed_unit_log;
mod last_shutdown_reading;
mod running_job_reading;

use boot_timeline_reading::StartupUnits;
use failed_unit_log::failed_units_item;
use running_job_reading::slow_jobs_item;
use sottomano_desktop_sinks::systemd_units_journal::systemd_unit_restarts::restart_failed_unit;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        SystemdOnBus, SystemdUnitScope,
        boot_timeline::{BootTimeline, BootTimelineWatch, StartupSlowdownThresholds},
        failed_units::{FailedUnitRestart, FailedUnits},
        journal_previous_shutdown::LastShutdown,
        slow_running_jobs::{SlowJobs, SlowJobsWatch},
    },
};
use sottomano_toml_config::{Config, widget_groups::systemd_manager::SystemdManagerConfig};

use crate::{
    background_work::{copy_to_clipboard, snapshot_subscription},
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

fn unit_scope_icon(scope: SystemdUnitScope) -> Phosphor {
    match scope {
        SystemdUnitScope::UserSession => Phosphor::User(Weight::Regular),
        SystemdUnitScope::SystemWide => Phosphor::HardDrives(Weight::Regular),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemdTarget {
    BootPhases,
    StartupUnits,
    FailedSystemUnits,
    FailedUserUnits,
    FailedSystemUnit(String),
    FailedUserUnit(String),
    SlowSystemJobs,
    SlowUserJobs,
    SlowSystemJob(String),
    SlowUserJob(String),
    LastShutdown,
}

impl SystemdTarget {
    fn failed_units(scope: SystemdUnitScope) -> Self {
        match scope {
            SystemdUnitScope::UserSession => SystemdTarget::FailedUserUnits,
            SystemdUnitScope::SystemWide => SystemdTarget::FailedSystemUnits,
        }
    }

    fn slow_jobs(scope: SystemdUnitScope) -> Self {
        match scope {
            SystemdUnitScope::UserSession => SystemdTarget::SlowUserJobs,
            SystemdUnitScope::SystemWide => SystemdTarget::SlowSystemJobs,
        }
    }

    fn failed_unit(scope: SystemdUnitScope, name: String) -> Self {
        match scope {
            SystemdUnitScope::UserSession => SystemdTarget::FailedUserUnit(name),
            SystemdUnitScope::SystemWide => SystemdTarget::FailedSystemUnit(name),
        }
    }

    fn slow_job(scope: SystemdUnitScope, name: String) -> Self {
        match scope {
            SystemdUnitScope::UserSession => SystemdTarget::SlowUserJob(name),
            SystemdUnitScope::SystemWide => SystemdTarget::SlowSystemJob(name),
        }
    }
}

impl From<SystemdTarget> for HoverTarget {
    fn from(target: SystemdTarget) -> Self {
        HoverTarget::Systemd(target)
    }
}

#[derive(Default)]
pub struct SystemdManagerGroup {
    boot: LatestSnapshot<BootTimeline, HidesFetchFailures>,
    failed_system: LatestSnapshot<FailedUnits>,
    failed_user: LatestSnapshot<FailedUnits>,
    slow_system_jobs: LatestSnapshot<SlowJobs>,
    slow_user_jobs: LatestSnapshot<SlowJobs>,
    last_shutdown: LatestSnapshot<LastShutdown>,
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
        let systemd = |scope| SystemdOnBus {
            buses: buses.clone(),
            scope,
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
            )) => return Task::future(restart_failed_unit(restart)).discard(),
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

    #[must_use]
    pub fn hover_card_for(&self, target: &SystemdTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            SystemdTarget::BootPhases => self.boot.hover_card(clock),
            SystemdTarget::StartupUnits => self.boot.hover_card_with(
                (StartupUnits {
                    timeline: self.boot.get()?,
                })
                .hover_card(clock),
            ),
            SystemdTarget::FailedSystemUnits => self.failed_system.hover_card(clock),
            SystemdTarget::FailedUserUnits => self.failed_user.hover_card(clock),
            SystemdTarget::SlowSystemJobs => self.slow_system_jobs.hover_card(clock),
            SystemdTarget::SlowUserJobs => self.slow_user_jobs.hover_card(clock),
            SystemdTarget::LastShutdown => self.last_shutdown.hover_card(clock),
            SystemdTarget::FailedSystemUnit(_)
            | SystemdTarget::FailedUserUnit(_)
            | SystemdTarget::SlowSystemJob(_)
            | SystemdTarget::SlowUserJob(_) => None,
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, SystemdManagerInteraction> {
        let boot = self.boot.get();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::SystemdManager,
            &self.expansion,
            SystemdManagerInteraction::Expansion,
            [
                boot.map(|boot| {
                    GroupItem::reading(boot, SystemdTarget::BootPhases).previewed_on_handle()
                }),
                boot.map(|timeline| {
                    GroupItem::reading(StartupUnits { timeline }, SystemdTarget::StartupUnits)
                }),
                self.slow_system_jobs
                    .visible_reading()
                    .map(|jobs| slow_jobs_item(jobs, config, clock))
                    .or_else(|| {
                        self.slow_system_jobs
                            .group_item(SystemdTarget::SlowSystemJobs)
                    }),
                self.slow_user_jobs
                    .visible_reading()
                    .map(|jobs| slow_jobs_item(jobs, config, clock))
                    .or_else(|| self.slow_user_jobs.group_item(SystemdTarget::SlowUserJobs)),
                self.failed_system
                    .visible_reading()
                    .map(|failed| failed_units_item(failed, config, clock))
                    .or_else(|| {
                        self.failed_system
                            .group_item(SystemdTarget::FailedSystemUnits)
                    }),
                self.failed_user
                    .visible_reading()
                    .map(|failed| failed_units_item(failed, config, clock))
                    .or_else(|| self.failed_user.group_item(SystemdTarget::FailedUserUnits)),
                self.last_shutdown
                    .get()
                    .map(|last| {
                        GroupItem::reading(last, SystemdTarget::LastShutdown).on_press(
                            TerminalCommand::JOURNAL_PRESS_HINT,
                            SystemdManagerInteraction::OpenLog(TerminalCommand::journal(
                                config,
                                last.follow_log_args(),
                            )),
                        )
                    })
                    .or_else(|| self.last_shutdown.group_item(SystemdTarget::LastShutdown)),
            ]
            .into_iter()
            .flatten(),
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::Speedometer(Weight::Regular).into(),
            reading: self,
        })
    }

    fn current_health(&self) -> [Option<&dyn GroupItemReading>; 4] {
        [
            self.slow_system_jobs
                .visible_reading()
                .map(|visible_reading| visible_reading as &dyn GroupItemReading),
            self.slow_user_jobs
                .visible_reading()
                .map(|visible_reading| visible_reading as &dyn GroupItemReading),
            self.failed_system
                .visible_reading()
                .map(|visible_reading| visible_reading as &dyn GroupItemReading),
            self.failed_user
                .visible_reading()
                .map(|visible_reading| visible_reading as &dyn GroupItemReading),
        ]
    }
}

impl GroupItemReading for SystemdManagerGroup {
    fn name(&self) -> Cow<'static, str> {
        "Systemd".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Speedometer(Weight::Regular).into()
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        if self.boot.get().is_none() {
            return ReadingDisplayState::Pending;
        }
        self.current_health()
            .into_iter()
            .flatten()
            .map(|reading| reading.state(clock).severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let slow_jobs_count = |jobs: &SlowJobs| {
            LabeledValue::new(jobs.name().into_owned(), Quantity::count(jobs.jobs.len()))
        };
        let failed_units_count = |failed: &FailedUnits| {
            LabeledValue::new(
                failed.name().into_owned(),
                Quantity::count(failed.units.len()),
            )
        };
        HoverCardBody::dictionary(
            [
                self.slow_system_jobs.visible_reading().map(slow_jobs_count),
                self.slow_user_jobs.visible_reading().map(slow_jobs_count),
                self.failed_system.visible_reading().map(failed_units_count),
                self.failed_user.visible_reading().map(failed_units_count),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
