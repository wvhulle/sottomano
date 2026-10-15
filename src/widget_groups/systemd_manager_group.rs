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
        Delivery,
        previous_shutdown::LastShutdown,
        systemd_manager::{
            System, UnitScope, User,
            boot_timeline::BootTimeline,
            failed_units::{Failed, FailedUnitRestart},
            running_jobs::SlowJobs,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{copy_to_clipboard, listening},
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

pub trait ScopeAppearance: UnitScope {
    const ICON: Phosphor;
    const SCOPE_NAME: &'static str;
    const FAILED_LABEL: &'static str;
    const FAILED_TARGET: SystemdTarget;
    const SLOW_JOBS_LABEL: &'static str;
    const SLOW_JOBS_TARGET: SystemdTarget;

    fn failed_unit_target(name: String) -> SystemdTarget;

    fn slow_job_target(name: String) -> SystemdTarget;
}

impl ScopeAppearance for User {
    const ICON: Phosphor = Phosphor::User(Weight::Regular);
    const SCOPE_NAME: &'static str = "user";
    const FAILED_LABEL: &'static str = "Failed user units";
    const FAILED_TARGET: SystemdTarget = SystemdTarget::FailedUserUnits;
    const SLOW_JOBS_LABEL: &'static str = "Slow user jobs";
    const SLOW_JOBS_TARGET: SystemdTarget = SystemdTarget::SlowUserJobs;

    fn failed_unit_target(name: String) -> SystemdTarget {
        SystemdTarget::FailedUserUnit(name)
    }

    fn slow_job_target(name: String) -> SystemdTarget {
        SystemdTarget::SlowUserJob(name)
    }
}

impl ScopeAppearance for System {
    const ICON: Phosphor = Phosphor::HardDrives(Weight::Regular);
    const SCOPE_NAME: &'static str = "system";
    const FAILED_LABEL: &'static str = "Failed system units";
    const FAILED_TARGET: SystemdTarget = SystemdTarget::FailedSystemUnits;
    const SLOW_JOBS_LABEL: &'static str = "Slow system jobs";
    const SLOW_JOBS_TARGET: SystemdTarget = SystemdTarget::SlowSystemJobs;

    fn failed_unit_target(name: String) -> SystemdTarget {
        SystemdTarget::FailedSystemUnit(name)
    }

    fn slow_job_target(name: String) -> SystemdTarget {
        SystemdTarget::SlowSystemJob(name)
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
    failed_system: ContinuousReading<Failed<System>>,
    failed_user: ContinuousReading<Failed<User>>,
    slow_system_jobs: ContinuousReading<SlowJobs<System>>,
    slow_user_jobs: ContinuousReading<SlowJobs<User>>,
    last_shutdown: ContinuousReading<LastShutdown>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum SystemdManagerMessage {
    Boot(Delivery<BootTimeline>),
    FailedSystem(Delivery<Failed<System>>),
    FailedUser(Delivery<Failed<User>>),
    SlowSystemJobs(Delivery<SlowJobs<System>>),
    SlowUserJobs(Delivery<SlowJobs<User>>),
    LastShutdown(Delivery<LastShutdown>),
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
            listening::<BootTimeline>(config.slow_boot_unit).map(SystemdManagerMessage::Boot),
            listening::<Failed<System>>(()).map(SystemdManagerMessage::FailedSystem),
            listening::<Failed<User>>(()).map(SystemdManagerMessage::FailedUser),
            listening::<SlowJobs<System>>(config.slow_job)
                .map(SystemdManagerMessage::SlowSystemJobs),
            listening::<SlowJobs<User>>(config.slow_job).map(SystemdManagerMessage::SlowUserJobs),
            listening::<LastShutdown>(config.slow_shutdown_stop)
                .map(SystemdManagerMessage::LastShutdown),
        ])
    }

    pub fn update(&mut self, msg: SystemdManagerMessage) -> Task<SystemdManagerMessage> {
        match msg {
            SystemdManagerMessage::Boot(delivery) => self.boot.push_or_keep(delivery),
            SystemdManagerMessage::FailedSystem(delivery) => {
                self.failed_system.push_or_keep(delivery);
            }
            SystemdManagerMessage::FailedUser(delivery) => self.failed_user.push_or_keep(delivery),
            SystemdManagerMessage::SlowSystemJobs(delivery) => {
                self.slow_system_jobs.push_or_keep(delivery);
            }
            SystemdManagerMessage::SlowUserJobs(delivery) => {
                self.slow_user_jobs.push_or_keep(delivery);
            }
            SystemdManagerMessage::LastShutdown(delivery) => {
                self.last_shutdown.push_or_keep(delivery);
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
            SystemdTarget::FailedSystemUnits => self.failed_system.present_tooltip(),
            SystemdTarget::FailedUserUnits => self.failed_user.present_tooltip(),
            SystemdTarget::SlowSystemJobs => self.slow_system_jobs.present_tooltip(),
            SystemdTarget::SlowUserJobs => self.slow_user_jobs.present_tooltip(),
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
                    .present_reading()
                    .map(|jobs| jobs.group_item(config)),
                self.slow_user_jobs
                    .present_reading()
                    .map(|jobs| jobs.group_item(config)),
                self.failed_system
                    .present_reading()
                    .map(|failed| failed.group_item(config)),
                self.failed_user
                    .present_reading()
                    .map(|failed| failed.group_item(config)),
                self.last_shutdown.get().map(|last| last.group_item(config)),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::IconColoredBy {
            glyph: Phosphor::Heartbeat(Weight::Regular).into(),
            reading: self,
        })
        .preview(boot.map(|boot| PreviewedValue::Hoverable {
            widget: boot,
            target: SystemdTarget::BootPhases.into(),
        }))
    }

    fn current_health(&self) -> [Option<&dyn BarWidget>; 4] {
        [
            self.slow_system_jobs
                .present_reading()
                .map(|w| w as &dyn BarWidget),
            self.slow_user_jobs
                .present_reading()
                .map(|w| w as &dyn BarWidget),
            self.failed_system
                .present_reading()
                .map(|w| w as &dyn BarWidget),
            self.failed_user
                .present_reading()
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
                    System::SLOW_JOBS_LABEL,
                    self.slow_system_jobs
                        .present_reading()
                        .map(|jobs| jobs.jobs.len()),
                ),
                count(
                    User::SLOW_JOBS_LABEL,
                    self.slow_user_jobs
                        .present_reading()
                        .map(|jobs| jobs.jobs.len()),
                ),
                count(
                    System::FAILED_LABEL,
                    self.failed_system
                        .present_reading()
                        .map(|failed| failed.units.len()),
                ),
                count(
                    User::FAILED_LABEL,
                    self.failed_user
                        .present_reading()
                        .map(|failed| failed.units.len()),
                ),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
