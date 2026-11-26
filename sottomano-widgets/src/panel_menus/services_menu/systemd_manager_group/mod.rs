use std::borrow::Cow;

use iced::{Subscription, Task};

use self::{
    failed_units_indicator::{FailedUnitsScope, failed_units_listing},
    slow_jobs_indicator::slow_jobs_listing,
};
use crate::{clock::Clock, group_indicators::indicator_group_identity::IndicatorGroupFamily};

mod boot_timeline_indicator;
mod failed_units_indicator;
mod last_shutdown_indicator;
mod slow_jobs_indicator;

use failed_units_indicator::{failed_units_panel_visibility, failed_units_state};
use slow_jobs_indicator::slow_jobs_panel_visibility;
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
use sottomano_toml_config::{
    Config, panel_menus::services_menu::systemd_manager::SystemdManagerConfig,
};

use crate::{
    background_work::{copy_to_clipboard, logged_failure_task, snapshot_subscription},
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupIdentity,
        indicator_state::IndicatorState,
        terminal_command::TerminalCommand,
    },
    hover_cards::IndicatorKey,
    widgets::{ongoing_toast::OngoingKey, subscribed_snapshot::SubscribedSnapshot},
};

fn unit_scope_icon(scope: SystemdUnitScope) -> Phosphor {
    match scope {
        SystemdUnitScope::UserSession => Phosphor::User(Weight::Regular),
        SystemdUnitScope::SystemWide => Phosphor::HardDrives(Weight::Regular),
    }
}

fn unit_toast_key(row_name: Cow<'static, str>, unit_name: String) -> OngoingKey {
    OngoingKey::SampleDetail {
        indicator: IndicatorKey {
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::SystemdManager),
            name: row_name,
        },
        detail: Cow::Owned(unit_name),
    }
}

#[derive(Default)]
pub struct SystemdManagerGroup {
    boot: SubscribedSnapshot<BootTimeline>,
    failed_system: SubscribedSnapshot<FailedUnits>,
    failed_user: SubscribedSnapshot<FailedUnits>,
    slow_system_jobs: SubscribedSnapshot<SlowJobs>,
    slow_user_jobs: SubscribedSnapshot<SlowJobs>,
    last_shutdown: SubscribedSnapshot<LastShutdown>,
    expansion: IndicatorGroupExpansion,
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
    Expansion(IndicatorGroupExpansionMessage),
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

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'view> {
        let indicator_listing = [
            self.boot_timeline_listing(config.calendar.locale),
            slow_jobs_listing(&self.slow_system_jobs, config, clock),
            slow_jobs_listing(&self.slow_user_jobs, config, clock),
            failed_units_listing(&self.failed_system, FailedUnitsScope::System, config, clock),
            failed_units_listing(&self.failed_user, FailedUnitsScope::User, config, clock),
            self.last_shutdown_listing(config),
        ]
        .into_iter()
        .collect();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::SystemdManager,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::SystemdManager.title(),
                Phosphor::Speedometer(Weight::Regular).into(),
                self.state(),
            ),
            &self.expansion,
            SystemdManagerInteraction::Expansion,
            indicator_listing,
        )
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
