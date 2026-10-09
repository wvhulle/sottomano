use std::time::Duration;

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
        previous_shutdown::{LastShutdown, read_last_shutdown},
        systemd_manager::{
            System, UnitScope, User,
            boot_timeline::{BootTimeline, boot_timeline_stream},
            failed_units::{Failed, failed_stream},
            running_jobs::{SlowJobs, slow_jobs_stream},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::journal_subscription,
    widget_groups::terminal_journal::TerminalJournal,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
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
    const FAILED_TARGET: HoverTarget;
    const SLOW_JOBS_LABEL: &'static str;
    const SLOW_JOBS_TARGET: HoverTarget;

    fn failed_unit_target(name: String) -> HoverTarget;

    fn slow_job_target(name: String) -> HoverTarget;
}

impl ScopeAppearance for User {
    const ICON: Phosphor = Phosphor::User(Weight::Regular);
    const SCOPE_NAME: &'static str = "user";
    const FAILED_LABEL: &'static str = "Failed user units";
    const FAILED_TARGET: HoverTarget = HoverTarget::SystemdFailedUserUnits;
    const SLOW_JOBS_LABEL: &'static str = "Slow user jobs";
    const SLOW_JOBS_TARGET: HoverTarget = HoverTarget::SystemdSlowUserJobs;

    fn failed_unit_target(name: String) -> HoverTarget {
        HoverTarget::FailedUserUnit(name)
    }

    fn slow_job_target(name: String) -> HoverTarget {
        HoverTarget::SlowUserJob(name)
    }
}

impl ScopeAppearance for System {
    const ICON: Phosphor = Phosphor::HardDrives(Weight::Regular);
    const SCOPE_NAME: &'static str = "system";
    const FAILED_LABEL: &'static str = "Failed system units";
    const FAILED_TARGET: HoverTarget = HoverTarget::SystemdFailedSystemUnits;
    const SLOW_JOBS_LABEL: &'static str = "Slow system jobs";
    const SLOW_JOBS_TARGET: HoverTarget = HoverTarget::SystemdSlowSystemJobs;

    fn failed_unit_target(name: String) -> HoverTarget {
        HoverTarget::FailedSystemUnit(name)
    }

    fn slow_job_target(name: String) -> HoverTarget {
        HoverTarget::SlowSystemJob(name)
    }
}

#[derive(Default)]
pub struct SystemdManagerGroup {
    boot: LastPoll<BootTimeline>,
    failed_system: LastPoll<Failed<System>>,
    failed_user: LastPoll<Failed<User>>,
    slow_system_jobs: LastPoll<SlowJobs<System>>,
    slow_user_jobs: LastPoll<SlowJobs<User>>,
    last_shutdown: LastPoll<LastShutdown>,
    coredumps: LastPoll<Coredumps>,
    oom_kills: LastPoll<OomKills>,
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
                    stream::once(spawn_blocking(move || read_last_shutdown(slow_stop)))
                        .filter_map(|read| {
                            ready(match read {
                                Ok(Ok(last_shutdown)) => Some(last_shutdown),
                                Ok(Err(e)) => {
                                    tracing::warn!("systemd last shutdown: {e}");
                                    None
                                }
                                Err(e) => {
                                    tracing::error!("systemd last shutdown: task panicked: {e}");
                                    None
                                }
                            })
                        })
                        .map(SystemdManagerMessage::LastShutdown)
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
            SystemdManagerMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::SystemdBootPhases => self.boot.get()?.phases.tooltip(),
            HoverTarget::SystemdSystemStartup => self.boot.get()?.system_startup.tooltip(),
            HoverTarget::SystemdUserStartup => self.boot.get()?.user_startup.as_ref()?.tooltip(),
            HoverTarget::SystemdFailedSystemUnits => self.failed_system.get()?.tooltip(),
            HoverTarget::SystemdFailedUserUnits => self.failed_user.get()?.tooltip(),
            HoverTarget::SystemdSlowSystemJobs => self.slow_system_jobs.get()?.tooltip(),
            HoverTarget::SystemdSlowUserJobs => self.slow_user_jobs.get()?.tooltip(),
            HoverTarget::SystemdLastShutdown => self.last_shutdown.get()?.tooltip(),
            HoverTarget::SystemdCoredumps => self.coredumps.get()?.tooltip(),
            HoverTarget::SystemdOomKills => self.oom_kills.get()?.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroup<'a, SystemdManagerMessage> {
        let open_log =
            |args| SystemdManagerMessage::OpenLog(TerminalJournal::following(config, args));
        let boot = self.boot.get();
        let boot_phases = boot.map(|boot| &boot.phases as &dyn BarWidget);
        WidgetGroup::new(&self.expansion, SystemdManagerMessage::Expansion)
            .icon_colored_by(Phosphor::Heartbeat(Weight::Regular), self)
            .previewed_reading(boot_phases, HoverTarget::SystemdBootPhases)
            .reading(boot_phases, HoverTarget::SystemdBootPhases)
            .reading(
                boot.map(|boot| &boot.system_startup as &dyn BarWidget),
                HoverTarget::SystemdSystemStartup,
            )
            .reading(
                boot.and_then(|boot| boot.user_startup.as_ref())
                    .map(|user| user as &dyn BarWidget),
                HoverTarget::SystemdUserStartup,
            )
            .item(
                self.slow_system_jobs
                    .get()
                    .map(|jobs| jobs.group_item(config)),
            )
            .item(
                self.slow_user_jobs
                    .get()
                    .map(|jobs| jobs.group_item(config)),
            )
            .item(
                self.failed_system
                    .get()
                    .map(|failed| failed.group_item(config)),
            )
            .item(
                self.failed_user
                    .get()
                    .map(|failed| failed.group_item(config)),
            )
            .item(self.oom_kills.get().map(|oom_kills| {
                GroupItem::reading(oom_kills, HoverTarget::SystemdOomKills).on_press(
                    TerminalJournal::PRESS_HINT,
                    open_log(OomKills::follow_log_args()),
                )
            }))
            .item(self.coredumps.get().map(|coredumps| {
                GroupItem::reading(coredumps, HoverTarget::SystemdCoredumps).on_press(
                    TerminalJournal::PRESS_HINT,
                    open_log(Coredumps::follow_log_args()),
                )
            }))
            .item(self.last_shutdown.get().map(|last_shutdown| {
                GroupItem::reading(last_shutdown, HoverTarget::SystemdLastShutdown).on_press(
                    TerminalJournal::PRESS_HINT,
                    open_log(last_shutdown.follow_log_args()),
                )
            }))
            .standalone_alerts(
                self.slow_system_jobs
                    .get()
                    .into_iter()
                    .flat_map(|jobs| jobs.job_alerts(open_log, config.locale)),
            )
            .standalone_alerts(
                self.slow_user_jobs
                    .get()
                    .into_iter()
                    .flat_map(|jobs| jobs.job_alerts(open_log, config.locale)),
            )
            .standalone_alerts(
                self.failed_system
                    .get()
                    .into_iter()
                    .flat_map(|failed| failed.unit_alerts(open_log, config.locale)),
            )
            .standalone_alerts(
                self.failed_user
                    .get()
                    .into_iter()
                    .flat_map(|failed| failed.unit_alerts(open_log, config.locale)),
            )
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
        TooltipBody::rows(
            [
                self.slow_system_jobs.get().map(SlowJobs::count_row),
                self.slow_user_jobs.get().map(SlowJobs::count_row),
                self.failed_system.get().map(Failed::count_row),
                self.failed_user.get().map(Failed::count_row),
                self.oom_kills.get().map(OomKills::count_row),
                self.coredumps.get().map(Coredumps::count_row),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
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
