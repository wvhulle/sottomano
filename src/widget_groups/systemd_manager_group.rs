use std::time::Duration;

use iced::{Subscription, Task};

mod boot_timeline_reading;
mod crashed_unit_actions;
mod failed_unit_log;
mod last_shutdown_reading;
mod running_job_reading;

use crate::{
    config::{Config, widget_groups::systemd_manager::SystemdManagerConfig},
    data::source::{
        SourceDelivery,
        previous_shutdown::LastShutdown,
        systemd_manager::{
            SystemdUnitScope,
            boot_timeline::BootTimeline,
            failed_units::{FailedUnitRestart, FailedUnits},
            running_jobs::{SlowJobs, SlowJobsWatch},
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{copy_to_clipboard, source_subscription},
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

pub fn elapsed_text(duration: Duration) -> String {
    if duration < WHOLE_SECONDS_FROM {
        format!("{:.1} s", duration.as_secs_f64())
    } else {
        humantime::format_duration(Duration::from_secs(duration.as_secs())).to_string()
    }
}

impl SystemdUnitScope {
    pub const fn scope_icon(self) -> Phosphor {
        match self {
            SystemdUnitScope::UserSession => Phosphor::User(Weight::Regular),
            SystemdUnitScope::SystemWide => Phosphor::HardDrives(Weight::Regular),
        }
    }

    pub const fn scope_name(self) -> &'static str {
        match self {
            SystemdUnitScope::UserSession => "user",
            SystemdUnitScope::SystemWide => "system",
        }
    }

    pub const fn failed_units_label(self) -> &'static str {
        match self {
            SystemdUnitScope::UserSession => "Failed user units",
            SystemdUnitScope::SystemWide => "Failed system units",
        }
    }

    pub const fn slow_jobs_label(self) -> &'static str {
        match self {
            SystemdUnitScope::UserSession => "Slow user jobs",
            SystemdUnitScope::SystemWide => "Slow system jobs",
        }
    }

    pub const fn failed_units_target(self) -> SystemdTarget {
        match self {
            SystemdUnitScope::UserSession => SystemdTarget::FailedUserUnits,
            SystemdUnitScope::SystemWide => SystemdTarget::FailedSystemUnits,
        }
    }

    pub const fn slow_jobs_target(self) -> SystemdTarget {
        match self {
            SystemdUnitScope::UserSession => SystemdTarget::SlowUserJobs,
            SystemdUnitScope::SystemWide => SystemdTarget::SlowSystemJobs,
        }
    }

    pub fn failed_unit_target(self, name: String) -> SystemdTarget {
        match self {
            SystemdUnitScope::UserSession => SystemdTarget::FailedUserUnit(name),
            SystemdUnitScope::SystemWide => SystemdTarget::FailedSystemUnit(name),
        }
    }

    pub fn slow_job_target(self, name: String) -> SystemdTarget {
        match self {
            SystemdUnitScope::UserSession => SystemdTarget::SlowUserJob(name),
            SystemdUnitScope::SystemWide => SystemdTarget::SlowSystemJob(name),
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

    pub fn subscription(config: &SystemdManagerConfig) -> Subscription<SystemdManagerMessage> {
        Subscription::batch([
            source_subscription::<BootTimeline>(config.slow_boot_unit, None)
                .map(SystemdManagerMessage::Boot),
            source_subscription::<FailedUnits>(SystemdUnitScope::SystemWide, None)
                .map(SystemdManagerMessage::FailedSystem),
            source_subscription::<FailedUnits>(SystemdUnitScope::UserSession, None)
                .map(SystemdManagerMessage::FailedUser),
            source_subscription::<SlowJobs>(
                SlowJobsWatch {
                    scope: SystemdUnitScope::SystemWide,
                    slow_after: config.slow_job,
                },
                None,
            )
            .map(SystemdManagerMessage::SlowSystemJobs),
            source_subscription::<SlowJobs>(
                SlowJobsWatch {
                    scope: SystemdUnitScope::UserSession,
                    slow_after: config.slow_job,
                },
                None,
            )
            .map(SystemdManagerMessage::SlowUserJobs),
            source_subscription::<LastShutdown>(config.slow_shutdown_stop, None)
                .map(SystemdManagerMessage::LastShutdown),
        ])
    }

    pub fn update(&mut self, msg: SystemdManagerMessage) -> Task<SystemdManagerMessage> {
        match msg {
            SystemdManagerMessage::Boot(delivery) => {
                self.boot.receive_keeping_last_reading(delivery);
            }
            SystemdManagerMessage::FailedSystem(delivery) => {
                self.failed_system.receive_keeping_last_reading(delivery);
            }
            SystemdManagerMessage::FailedUser(delivery) => {
                self.failed_user.receive_keeping_last_reading(delivery);
            }
            SystemdManagerMessage::SlowSystemJobs(delivery) => {
                self.slow_system_jobs.receive_keeping_last_reading(delivery);
            }
            SystemdManagerMessage::SlowUserJobs(delivery) => {
                self.slow_user_jobs.receive_keeping_last_reading(delivery);
            }
            SystemdManagerMessage::LastShutdown(delivery) => {
                self.last_shutdown.receive_keeping_last_reading(delivery);
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

    pub fn tooltip_for(&self, target: &SystemdTarget) -> Option<Tooltip> {
        match target {
            SystemdTarget::BootPhases => self.boot.tooltip(),
            SystemdTarget::StartupUnits => {
                self.boot.tooltip_with(self.boot.get()?.units().tooltip())
            }
            SystemdTarget::FailedSystemUnits => self.failed_system.tooltip(),
            SystemdTarget::FailedUserUnits => self.failed_user.tooltip(),
            SystemdTarget::SlowSystemJobs => self.slow_system_jobs.tooltip(),
            SystemdTarget::SlowUserJobs => self.slow_user_jobs.tooltip(),
            SystemdTarget::LastShutdown => self.last_shutdown.tooltip(),
            SystemdTarget::FailedSystemUnit(_)
            | SystemdTarget::FailedUserUnit(_)
            | SystemdTarget::SlowSystemJob(_)
            | SystemdTarget::SlowUserJob(_) => None,
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, SystemdManagerMessage> {
        let boot = self.boot.get();
        GroupContent::new(
            &self.expansion,
            SystemdManagerMessage::Expansion,
            [
                boot.map(|boot| GroupItem::reading(boot, SystemdTarget::BootPhases)),
                boot.map(|boot| GroupItem::reading(boot.units(), SystemdTarget::StartupUnits)),
                self.slow_system_jobs
                    .visible_reading()
                    .map(|jobs| jobs.group_item(config)),
                self.slow_user_jobs
                    .visible_reading()
                    .map(|jobs| jobs.group_item(config)),
                self.failed_system
                    .visible_reading()
                    .map(|failed| failed.group_item(config)),
                self.failed_user
                    .visible_reading()
                    .map(|failed| failed.group_item(config)),
                self.last_shutdown.get().map(|last| last.group_item(config)),
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
        Phosphor::Heartbeat(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.boot.get().is_none() {
            return WidgetState::Pending;
        }
        self.current_health()
            .into_iter()
            .flatten()
            .map(|reading| reading.state().severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let count =
            |key, count: Option<usize>| count.map(|count| KeyValue::new(key, count.to_string()));
        TooltipBody::dictionary(
            [
                count(
                    SystemdUnitScope::SystemWide.slow_jobs_label(),
                    self.slow_system_jobs
                        .visible_reading()
                        .map(|jobs| jobs.jobs.len()),
                ),
                count(
                    SystemdUnitScope::UserSession.slow_jobs_label(),
                    self.slow_user_jobs
                        .visible_reading()
                        .map(|jobs| jobs.jobs.len()),
                ),
                count(
                    SystemdUnitScope::SystemWide.failed_units_label(),
                    self.failed_system
                        .visible_reading()
                        .map(|failed| failed.units.len()),
                ),
                count(
                    SystemdUnitScope::UserSession.failed_units_label(),
                    self.failed_user
                        .visible_reading()
                        .map(|failed| failed.units.len()),
                ),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
