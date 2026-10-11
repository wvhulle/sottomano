use std::{path::PathBuf, time::Duration};

use iced::{
    Subscription, Task,
    futures::{StreamExt, future::ready, stream},
};
use tokio::task::spawn_blocking;

pub mod boot_timeline_reading;
pub mod coredump_log;
pub mod failed_unit_log;
pub mod last_shutdown_reading;
pub mod oom_kill_log;
pub mod running_job_reading;

use crate::{
    config::{Config, systemd::SystemdConfig},
    data::source::{
        coredumps::Coredumps,
        oom_kills::OomKills,
        previous_shutdown::{LastShutdown, ShutdownReadError, read_last_shutdown},
        systemd_manager::{
            System, UnitScope, User,
            boot_timeline::{BootTimeline, boot_timeline_stream},
            failed_units::{Failed, FailedUnitRestart, failed_stream},
            running_jobs::{SlowJobs, slow_jobs_stream},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{copy_to_clipboard, journal_subscription},
    widget_groups::{terminal_command::TerminalCommand, terminal_journal::TerminalJournal},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
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
    Coredumps,
    OomKills,
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
    coredumps: ContinuousReading<Coredumps>,
    oom_kills: ContinuousReading<OomKills>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum SystemdManagerMessage {
    Boot(BootTimeline),
    FailedSystem(Option<Failed<System>>),
    FailedUser(Option<Failed<User>>),
    SlowSystemJobs(Option<SlowJobs<System>>),
    SlowUserJobs(Option<SlowJobs<User>>),
    LastShutdown(Option<LastShutdown>),
    Coredumps(Option<Coredumps>),
    OomKills(Option<OomKills>),
    OpenLog(TerminalJournal),
    RestartFailedUnit(FailedUnitRestart),
    InspectCoredump(TerminalCommand),
    CopyCoredumpPath(PathBuf),
    Expansion(ExpansionMessage),
}

impl SystemdManagerGroup {
    pub fn subscription(config: &SystemdConfig) -> Subscription<SystemdManagerMessage> {
        Subscription::batch([
            Subscription::run_with(
                ("systemd-boot-timeline", config.slow_boot_unit()),
                |&(_, slow_unit)| boot_timeline_stream(slow_unit).map(SystemdManagerMessage::Boot),
            ),
            failed_subscription::<System>(SystemdManagerMessage::FailedSystem),
            failed_subscription::<User>(SystemdManagerMessage::FailedUser),
            slow_jobs_subscription::<System>(
                config.slow_job(),
                SystemdManagerMessage::SlowSystemJobs,
            ),
            slow_jobs_subscription::<User>(config.slow_job(), SystemdManagerMessage::SlowUserJobs),
            Subscription::run_with(
                ("systemd-last-shutdown", config.slow_shutdown_stop()),
                |&(_, slow_stop)| {
                    stream::once(spawn_blocking(move || read_last_shutdown(slow_stop))).filter_map(
                        |read| {
                            ready(match read {
                                Ok(Ok(last_shutdown)) => {
                                    Some(SystemdManagerMessage::LastShutdown(Some(last_shutdown)))
                                }
                                Ok(Err(
                                    ShutdownReadError::NoPreviousBoot
                                    | ShutdownReadError::NoShutdownRecorded,
                                )) => Some(SystemdManagerMessage::LastShutdown(None)),
                                Ok(Err(e)) => {
                                    tracing::warn!("systemd last shutdown: {e}");
                                    None
                                }
                                Err(e) => {
                                    tracing::error!("systemd last shutdown: task panicked: {e}");
                                    None
                                }
                            })
                        },
                    )
                },
            ),
            journal_subscription(Coredumps::from_entries, SystemdManagerMessage::Coredumps),
            journal_subscription(OomKills::from_entries, SystemdManagerMessage::OomKills),
        ])
    }

    pub fn update(&mut self, msg: SystemdManagerMessage) -> Task<SystemdManagerMessage> {
        match msg {
            SystemdManagerMessage::Boot(boot) => self.boot.load(Some(boot)),
            SystemdManagerMessage::FailedSystem(reading) => self.failed_system.load(reading),
            SystemdManagerMessage::FailedUser(reading) => self.failed_user.load(reading),
            SystemdManagerMessage::SlowSystemJobs(reading) => self.slow_system_jobs.load(reading),
            SystemdManagerMessage::SlowUserJobs(reading) => self.slow_user_jobs.load(reading),
            SystemdManagerMessage::LastShutdown(reading) => self.last_shutdown.load(reading),
            SystemdManagerMessage::Coredumps(reading) => self.coredumps.load(reading),
            SystemdManagerMessage::OomKills(reading) => self.oom_kills.load(reading),
            SystemdManagerMessage::OpenLog(journal) => return journal.open(),
            SystemdManagerMessage::RestartFailedUnit(restart) => {
                return Task::future(restart.apply()).discard();
            }
            SystemdManagerMessage::InspectCoredump(command) => return command.open(),
            SystemdManagerMessage::CopyCoredumpPath(path) => {
                return copy_to_clipboard(path.display().to_string());
            }
            SystemdManagerMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &SystemdTarget) -> Option<Tooltip> {
        let body = match target {
            SystemdTarget::BootPhases => self.boot.get()?.tooltip(),
            SystemdTarget::StartupUnits => self.boot.get()?.units().tooltip(),
            SystemdTarget::FailedSystemUnits => self.failed_system.get()?.tooltip(),
            SystemdTarget::FailedUserUnits => self.failed_user.get()?.tooltip(),
            SystemdTarget::SlowSystemJobs => self.slow_system_jobs.get()?.tooltip(),
            SystemdTarget::SlowUserJobs => self.slow_user_jobs.get()?.tooltip(),
            SystemdTarget::LastShutdown => self.last_shutdown.get()?.tooltip(),
            SystemdTarget::Coredumps => self.coredumps.get()?.tooltip(),
            SystemdTarget::OomKills => self.oom_kills.get()?.tooltip(),
            SystemdTarget::FailedSystemUnit(_)
            | SystemdTarget::FailedUserUnit(_)
            | SystemdTarget::SlowSystemJob(_)
            | SystemdTarget::SlowUserJob(_) => return None,
        };
        Some(Tooltip::of(body))
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
                    .get()
                    .map(|jobs| jobs.group_item(config)),
                self.slow_user_jobs
                    .get()
                    .map(|jobs| jobs.group_item(config)),
                self.failed_system
                    .get()
                    .map(|failed| failed.group_item(config)),
                self.failed_user
                    .get()
                    .map(|failed| failed.group_item(config)),
                self.oom_kills.get().map(|kills| kills.group_item(config)),
                self.coredumps.get().map(|dumps| dumps.group_item(config)),
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

    fn current_health(&self) -> [Option<&dyn BarWidget>; 6] {
        [
            self.slow_system_jobs.get().map(|w| w as &dyn BarWidget),
            self.slow_user_jobs.get().map(|w| w as &dyn BarWidget),
            self.failed_system.get().map(|w| w as &dyn BarWidget),
            self.failed_user.get().map(|w| w as &dyn BarWidget),
            self.oom_kills.get().map(|w| w as &dyn BarWidget),
            self.coredumps.get().map(|w| w as &dyn BarWidget),
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
        TooltipBody::sections(&[Section::untitled_dictionary(
            [
                count(
                    System::SLOW_JOBS_LABEL,
                    self.slow_system_jobs.get().map(|jobs| jobs.jobs.len()),
                ),
                count(
                    User::SLOW_JOBS_LABEL,
                    self.slow_user_jobs.get().map(|jobs| jobs.jobs.len()),
                ),
                count(
                    System::FAILED_LABEL,
                    self.failed_system.get().map(|failed| failed.units.len()),
                ),
                count(
                    User::FAILED_LABEL,
                    self.failed_user.get().map(|failed| failed.units.len()),
                ),
                count(
                    "OOM kills",
                    self.oom_kills.get().map(|kills| kills.entries.len()),
                ),
                count(
                    "Coredumps",
                    self.coredumps.get().map(|dumps| dumps.entries.len()),
                ),
            ]
            .into_iter()
            .flatten(),
        )])
    }
}

fn failed_subscription<S: UnitScope + 'static>(
    map: fn(Option<Failed<S>>) -> SystemdManagerMessage,
) -> Subscription<SystemdManagerMessage> {
    Subscription::run_with(
        ("systemd-failed-units", S::PER_USER, map),
        |&(_, per_user, map)| {
            failed_stream(per_user).map(move |units| map(Failed::from_units(units)))
        },
    )
}

fn slow_jobs_subscription<S: UnitScope + 'static>(
    slow_after: Duration,
    map: fn(Option<SlowJobs<S>>) -> SystemdManagerMessage,
) -> Subscription<SystemdManagerMessage> {
    Subscription::run_with(
        ("systemd-slow-jobs", S::PER_USER, slow_after, map),
        |&(_, per_user, slow_after, map)| {
            slow_jobs_stream(per_user, slow_after).map(move |jobs| map(SlowJobs::from_jobs(jobs)))
        },
    )
}
