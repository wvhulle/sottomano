use std::time::Duration;

use iced::{Subscription, Task};

use crate::clock::Clock;

mod boot_timeline_reading;
mod failed_unit_log;
mod last_shutdown_reading;
mod running_job_reading;

use boot_timeline_reading::StartupUnits;
use failed_unit_log::failed_units_item;
use running_job_reading::slow_jobs_item;
use sottomano_config::{Config, widget_groups::systemd_manager::SystemdManagerConfig};
use sottomano_sources::{
    SourceDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        SystemdOnBus, SystemdUnitScope,
        boot_timeline::{BootTimeline, BootTimelineWatch},
        failed_units::{FailedUnitRestart, FailedUnits},
        journal_previous_shutdown::LastShutdown,
        slow_running_jobs::{SlowJobs, SlowJobsWatch},
    },
};

use crate::{
    background_work::{copy_to_clipboard, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::terminal_command::TerminalCommand,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
        widget_state::WidgetState,
    },
};

const WHOLE_SECONDS_FROM: Duration = Duration::from_secs(60);

#[must_use]
pub fn elapsed_text(duration: Duration) -> String {
    if duration < WHOLE_SECONDS_FROM {
        format!("{:.1} s", duration.as_secs_f64())
    } else {
        humantime::format_duration(Duration::from_secs(duration.as_secs())).to_string()
    }
}

impl From<SystemdUnitScope> for Phosphor {
    fn from(scope: SystemdUnitScope) -> Self {
        match scope {
            SystemdUnitScope::UserSession => Phosphor::User(Weight::Regular),
            SystemdUnitScope::SystemWide => Phosphor::HardDrives(Weight::Regular),
        }
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
    boot: ContinuousReading<BootTimeline>,
    failed_system: ContinuousReading<FailedUnits>,
    failed_user: ContinuousReading<FailedUnits>,
    slow_system_jobs: ContinuousReading<SlowJobs>,
    slow_user_jobs: ContinuousReading<SlowJobs>,
    last_shutdown: ContinuousReading<LastShutdown>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum SystemdManagerMessage {
    Boot(SourceDelivery<BootTimeline>),
    FailedSystem(SourceDelivery<FailedUnits>),
    FailedUser(SourceDelivery<FailedUnits>),
    SlowSystemJobs(SourceDelivery<SlowJobs>),
    SlowUserJobs(SourceDelivery<SlowJobs>),
    LastShutdown(SourceDelivery<LastShutdown>),
    OpenLog(TerminalCommand),
    RestartFailedUnit(FailedUnitRestart),
    InspectCoredump(TerminalCommand),
    CopyToClipboard(String),
    Expansion(ExpansionMessage),
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
            source_subscription::<BootTimeline>(BootTimelineWatch {
                buses: buses.clone(),
                slow_unit: config.slow_boot_unit,
            })
            .map(SystemdManagerMessage::Boot),
            source_subscription::<FailedUnits>(systemd(SystemdUnitScope::SystemWide))
                .map(SystemdManagerMessage::FailedSystem),
            source_subscription::<FailedUnits>(systemd(SystemdUnitScope::UserSession))
                .map(SystemdManagerMessage::FailedUser),
            source_subscription::<SlowJobs>(SlowJobsWatch {
                systemd: systemd(SystemdUnitScope::SystemWide),
                slow_after: config.slow_job,
            })
            .map(SystemdManagerMessage::SlowSystemJobs),
            source_subscription::<SlowJobs>(SlowJobsWatch {
                systemd: systemd(SystemdUnitScope::UserSession),
                slow_after: config.slow_job,
            })
            .map(SystemdManagerMessage::SlowUserJobs),
            source_subscription::<LastShutdown>(config.slow_shutdown_stop)
                .map(SystemdManagerMessage::LastShutdown),
        ])
    }

    pub fn update(
        &mut self,
        msg: SystemdManagerMessage,
        clock: Clock,
    ) -> Task<SystemdManagerMessage> {
        match msg {
            SystemdManagerMessage::Boot(delivery) => {
                self.boot.receive_keeping_last_reading(delivery, clock);
            }
            SystemdManagerMessage::FailedSystem(delivery) => {
                self.failed_system.receive_delivery(delivery, clock);
            }
            SystemdManagerMessage::FailedUser(delivery) => {
                self.failed_user.receive_delivery(delivery, clock);
            }
            SystemdManagerMessage::SlowSystemJobs(delivery) => {
                self.slow_system_jobs.receive_delivery(delivery, clock);
            }
            SystemdManagerMessage::SlowUserJobs(delivery) => {
                self.slow_user_jobs.receive_delivery(delivery, clock);
            }
            SystemdManagerMessage::LastShutdown(delivery) => {
                self.last_shutdown.receive_delivery(delivery, clock);
            }
            SystemdManagerMessage::OpenLog(journal) => return journal.open(),
            SystemdManagerMessage::RestartFailedUnit(restart) => {
                return Task::future(restart.apply()).discard();
            }
            SystemdManagerMessage::InspectCoredump(command) => return command.open(),
            SystemdManagerMessage::CopyToClipboard(text) => return copy_to_clipboard(text),
            SystemdManagerMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &SystemdTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            SystemdTarget::BootPhases => self.boot.tooltip(clock),
            SystemdTarget::StartupUnits => self.boot.tooltip_with(
                (StartupUnits {
                    timeline: self.boot.get()?,
                })
                .tooltip(clock),
            ),
            SystemdTarget::FailedSystemUnits => self.failed_system.tooltip(clock),
            SystemdTarget::FailedUserUnits => self.failed_user.tooltip(clock),
            SystemdTarget::SlowSystemJobs => self.slow_system_jobs.tooltip(clock),
            SystemdTarget::SlowUserJobs => self.slow_user_jobs.tooltip(clock),
            SystemdTarget::LastShutdown => self.last_shutdown.tooltip(clock),
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
    ) -> GroupContent<'a, SystemdManagerMessage> {
        let boot = self.boot.get();
        GroupContent::new(
            &self.expansion,
            SystemdManagerMessage::Expansion,
            [
                boot.map(|boot| GroupItem::reading(boot, SystemdTarget::BootPhases)),
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
                            SystemdManagerMessage::OpenLog(TerminalCommand::journal(
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
        .face(HandleFace::IconColoredBy {
            glyph: Phosphor::Heartbeat(Weight::Regular).into(),
            reading: self,
            handle: GroupHandle::Systemd,
        })
        .preview(boot.map(|boot| PreviewedValue::Hoverable {
            widget: boot,
            target: SystemdTarget::BootPhases.into(),
        }))
    }

    fn current_health(&self) -> [Option<&dyn BarWidget>; 4] {
        [
            self.slow_system_jobs
                .visible_reading()
                .map(|w| w as &dyn BarWidget),
            self.slow_user_jobs
                .visible_reading()
                .map(|w| w as &dyn BarWidget),
            self.failed_system
                .visible_reading()
                .map(|w| w as &dyn BarWidget),
            self.failed_user
                .visible_reading()
                .map(|w| w as &dyn BarWidget),
        ]
    }
}

impl BarWidget for SystemdManagerGroup {
    fn name(&self) -> &'static str {
        "Systemd"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Heartbeat(Weight::Regular).into()
    }

    fn state(&self, clock: Clock) -> WidgetState {
        if self.boot.get().is_none() {
            return WidgetState::Pending;
        }
        self.current_health()
            .into_iter()
            .flatten()
            .map(|reading| reading.state(clock).severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let slow_jobs_count =
            |jobs: &SlowJobs| KeyValue::new(jobs.name(), jobs.jobs.len().to_string());
        let failed_units_count =
            |failed: &FailedUnits| KeyValue::new(failed.name(), failed.units.len().to_string());
        TooltipBody::dictionary(
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
