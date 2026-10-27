mod break_log;
mod context_switch_rate;
mod daily_totals_gauge;
mod focus_run_reading;
mod focus_span_store;
mod focus_tracker;
mod session_face;
mod window_usage;

use chrono::{DateTime, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, widget_groups::focus_time::FocusTimeConfig};
use sottomano_sources::{
    desktop_notifications::notification_data::ActionTint,
    desktop_windows_apps::kitty_terminal_windows::WindowProcesses,
    login_session_presence::focus_time_ledger::{FocusSignal, ScreenEdge},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::{
        niri_window_group::niri_kdl_rules::HeatedWindow,
        session_focus_group::{
            break_log::BreakLog,
            context_switch_rate::ContextSwitchRate,
            daily_totals_gauge::DailyTotalsGauge,
            focus_run_reading::FocusRunReading,
            focus_tracker::{FocusTracker, TrackerMessage},
            session_face::SessionFace,
            window_usage::WindowUsage,
        },
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        ongoing_toast::OngoingToastActionButton,
        tooltip_body::{Freshness, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFocusTarget {
    Session,
    Breaks,
    FocusRun,
    DailyTotals,
    ContextSwitches,
}

impl From<SessionFocusTarget> for HoverTarget {
    fn from(target: SessionFocusTarget) -> Self {
        HoverTarget::SessionFocus(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakPledge {
    pub at: DateTime<Utc>,
}

pub struct SessionFocusGroup {
    tracker: FocusTracker,
    pledge: Option<BreakPledge>,
    expansion: GroupExpansion,
    daily_totals: DailyTotalsGauge,
}

#[derive(Debug, Clone)]
pub enum SessionFocusMessage {
    Tracker(TrackerMessage),
    BreakPledged,
    Expansion(ExpansionMessage),
}

impl SessionFocusGroup {
    #[must_use]
    pub fn started_at(config: &FocusTimeConfig, now: DateTime<Utc>) -> Self {
        let tracker = FocusTracker::started_at(now);
        let daily_totals = DailyTotalsGauge::new(&tracker.ledger().accounted(config), now);
        Self {
            tracker,
            pledge: None,
            expansion: GroupExpansion::default(),
            daily_totals,
        }
    }

    pub fn refresh_daily_totals(&mut self, config: &FocusTimeConfig, clock: Clock) {
        let now = clock.utc();
        if !self.daily_totals.computed_in_second_of(now) {
            self.recompute_daily_totals(config, now);
        }
    }

    fn recompute_daily_totals(&mut self, config: &FocusTimeConfig, now: DateTime<Utc>) {
        self.daily_totals = DailyTotalsGauge::new(&self.tracker.ledger().accounted(config), now);
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn load(clock: Clock) -> Task<SessionFocusMessage> {
        Task::perform(
            FocusTracker::load(clock.utc()),
            SessionFocusMessage::Tracker,
        )
    }

    pub fn subscription(config: &FocusTimeConfig) -> Subscription<SessionFocusMessage> {
        Subscription::run_with(config.idle_detection_delay, FocusTracker::idle_messages)
            .map(SessionFocusMessage::Tracker)
    }

    pub fn sleep_subscription(buses: &DesktopBuses) -> Subscription<SessionFocusMessage> {
        Subscription::run_with(buses.clone(), FocusTracker::sleep_messages)
            .map(SessionFocusMessage::Tracker)
    }

    pub fn screen_power_changed(
        &mut self,
        edge: ScreenEdge,
        config: &FocusTimeConfig,
        clock: Clock,
    ) -> Task<SessionFocusMessage> {
        self.update(
            SessionFocusMessage::Tracker(TrackerMessage::Signal(FocusSignal::ScreenPower(edge))),
            config,
            clock,
        )
    }

    pub fn refocus(
        &mut self,
        niri: &EventStreamState,
        processes: &WindowProcesses,
    ) -> Task<SessionFocusMessage> {
        self.tracker
            .refocus(niri, processes)
            .map_or_else(Task::none, |probe| {
                Task::perform(probe, SessionFocusMessage::Tracker)
            })
    }

    pub fn update(
        &mut self,
        message: SessionFocusMessage,
        config: &FocusTimeConfig,
        clock: Clock,
    ) -> Task<SessionFocusMessage> {
        match message {
            SessionFocusMessage::Tracker(message) => {
                let span_writes = self.tracker.update(message, config, clock.utc());
                self.recompute_daily_totals(config, clock.utc());
                Task::future(span_writes).discard()
            }
            SessionFocusMessage::BreakPledged => {
                self.pledge = Some(BreakPledge { at: clock.utc() });
                Task::none()
            }
            SessionFocusMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
                Task::none()
            }
        }
    }

    fn session_face<'a>(&'a self, config: &'a Config) -> SessionFace<'a> {
        SessionFace {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            pledge: self.pledge,
        }
    }

    fn break_log<'a>(&'a self, config: &'a Config) -> BreakLog<'a> {
        BreakLog {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
        }
    }

    fn focus_run_reading<'a>(&'a self, config: &'a Config) -> FocusRunReading<'a> {
        FocusRunReading {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
        }
    }

    fn context_switch_rate<'a>(&'a self, config: &'a Config) -> ContextSwitchRate<'a> {
        ContextSwitchRate {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
        }
    }

    #[must_use]
    pub fn heated_windows(
        &self,
        niri: &EventStreamState,
        config: &Config,
        clock: Clock,
    ) -> Vec<HeatedWindow> {
        let accounting = self
            .tracker
            .ledger()
            .accounted(&config.widget_groups.focus_time);
        let now = clock.utc();
        let mut windows: Vec<_> = niri.windows.windows.values().collect();
        windows.sort_by_key(|window| window.id);
        windows
            .into_iter()
            .filter_map(|window| {
                let title = window.title.as_deref()?;
                let color = accounting.switch_heat_color(window.id, &config.theme.palette, now)?;
                HeatedWindow::titled(
                    window.app_id.clone(),
                    title,
                    format!("{:x}", color.to_rgba8()),
                )
            })
            .collect()
    }

    #[must_use]
    pub fn window_usage_tooltip(&self, window: u64, config: &Config, clock: Clock) -> Tooltip {
        let usage = WindowUsage {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            window,
        };
        Tooltip::sourced(usage.body(clock.utc()), Freshness::Live)
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, SessionFocusMessage> {
        let session = GroupItem::reading(self.session_face(config), SessionFocusTarget::Session)
            .ongoing_toast_action_buttons([OngoingToastActionButton {
                button_label: "Taking a break".to_owned(),
                tint: ActionTint::Proceed,
                on_button_pressed: SessionFocusMessage::BreakPledged,
            }]);
        let breaks = GroupItem::reading(self.break_log(config), SessionFocusTarget::Breaks);
        let focus_run =
            GroupItem::reading(self.focus_run_reading(config), SessionFocusTarget::FocusRun);
        let totals = GroupItem::reading(&self.daily_totals, SessionFocusTarget::DailyTotals);
        let switches = GroupItem::reading(
            self.context_switch_rate(config),
            SessionFocusTarget::ContextSwitches,
        );
        GroupContent::new(
            &self.expansion,
            SessionFocusMessage::Expansion,
            [session, breaks, focus_run, totals, switches],
        )
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: SessionFocusTarget,
        config: &Config,
        clock: Clock,
    ) -> Tooltip {
        let body = match target {
            SessionFocusTarget::Session => self.session_face(config).tooltip(clock),
            SessionFocusTarget::Breaks => self.break_log(config).tooltip(clock),
            SessionFocusTarget::FocusRun => self.focus_run_reading(config).tooltip(clock),
            SessionFocusTarget::DailyTotals => self.daily_totals.tooltip(clock),
            SessionFocusTarget::ContextSwitches => self.context_switch_rate(config).tooltip(clock),
        };
        Tooltip::sourced(body, Freshness::Live)
    }
}
