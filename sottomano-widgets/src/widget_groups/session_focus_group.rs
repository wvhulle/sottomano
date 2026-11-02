mod break_log;
mod context_switch_rate;
mod daily_totals_gauge;
mod focus_run_reading;
mod focus_tracker;
mod session_face;
mod sqlite_focus_spans;
mod window_usage;

use chrono::{DateTime, TimeDelta, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, widget_groups::session_focus::SessionFocusConfig};
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_notifications::notification_data::ActionTint,
    desktop_windows_apps::kitty_terminal_windows::WindowProcesses,
    login_session_presence::{
        focus_time_ledger::{FocusSignal, ScreenEdge},
        session_power_actions::SessionAction,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{
        HoverTarget,
        hover_card_body::hover_card_source_delivery::{DeliveryTiming, HoverCard},
    },
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
        widget_group_content::{
            WidgetGroup, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::ongoing_toast::OngoingToastActionButton,
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
pub struct WarningSnooze {
    pub until: DateTime<Utc>,
}

pub struct SessionFocusGroup {
    tracker: FocusTracker,
    snooze: Option<WarningSnooze>,
    expansion: WidgetGroupExpansion,
    daily_totals: DailyTotalsGauge,
}

#[derive(Debug, Clone)]
pub enum SessionFocusMessage {
    Tracker(TrackerMessage),
    BreakStarted(SessionAction),
    WarningSnoozed,
    Expansion(WidgetGroupExpansionMessage),
}

impl WidgetGroup for SessionFocusGroup {
    const TITLE: &'static str = "Focus session";
}

impl SessionFocusGroup {
    #[must_use]
    pub fn started_at(config: &SessionFocusConfig, now: DateTime<Utc>) -> Self {
        let tracker = FocusTracker::started_at(now);
        let daily_totals = DailyTotalsGauge::new(&tracker.ledger().accounted(config), now);
        Self {
            tracker,
            snooze: None,
            expansion: WidgetGroupExpansion::default(),
            daily_totals,
        }
    }

    pub fn refresh_daily_totals(&mut self, config: &SessionFocusConfig, clock: Clock) {
        let now = clock.utc();
        if !self.daily_totals.computed_in_second_of(now) {
            self.recompute_daily_totals(config, now);
        }
    }

    fn recompute_daily_totals(&mut self, config: &SessionFocusConfig, now: DateTime<Utc>) {
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

    pub fn subscription(config: &SessionFocusConfig) -> Subscription<SessionFocusMessage> {
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
        config: &SessionFocusConfig,
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
        config: &SessionFocusConfig,
        clock: Clock,
    ) -> Task<SessionFocusMessage> {
        match message {
            SessionFocusMessage::Tracker(message) => {
                let span_writes = self.tracker.update(message, config, clock.utc());
                self.recompute_daily_totals(config, clock.utc());
                Task::future(span_writes).discard()
            }
            SessionFocusMessage::BreakStarted(action) => {
                Task::future(blocking("break session action", move || action.perform())).discard()
            }
            SessionFocusMessage::WarningSnoozed => {
                let snooze_length =
                    TimeDelta::from_std(config.warning_snooze).unwrap_or(TimeDelta::MAX);
                self.snooze = Some(WarningSnooze {
                    until: clock.utc() + snooze_length,
                });
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
                .accounted(&config.widget_groups.session_focus),
            snooze: self.snooze,
        }
    }

    fn break_log<'a>(&'a self, config: &'a Config) -> BreakLog<'a> {
        BreakLog {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.session_focus),
        }
    }

    fn focus_run_reading<'a>(&'a self, config: &'a Config) -> FocusRunReading<'a> {
        FocusRunReading {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.session_focus),
        }
    }

    fn context_switch_rate<'a>(&'a self, config: &'a Config) -> ContextSwitchRate<'a> {
        ContextSwitchRate {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.session_focus),
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
            .accounted(&config.widget_groups.session_focus);
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
    pub fn window_usage_hover_card(&self, window: u64, config: &Config, clock: Clock) -> HoverCard {
        let usage = WindowUsage {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.session_focus),
            window,
        };
        HoverCard::sourced(usage.body(clock.utc()), DeliveryTiming::LivePush)
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, SessionFocusMessage> {
        let session = GroupItem::reading(self.session_face(config), SessionFocusTarget::Session)
            .ongoing_toast_action_buttons([
                OngoingToastActionButton {
                    button_label: "Lock screen".to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: SessionFocusMessage::BreakStarted(SessionAction::Lock),
                },
                OngoingToastActionButton {
                    button_label: "Screen off".to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: SessionFocusMessage::BreakStarted(SessionAction::ScreenOff),
                },
                OngoingToastActionButton {
                    button_label: "Snooze".to_owned(),
                    tint: ActionTint::Hold,
                    on_button_pressed: SessionFocusMessage::WarningSnoozed,
                },
            ]);
        let breaks = GroupItem::reading(self.break_log(config), SessionFocusTarget::Breaks);
        let focus_run =
            GroupItem::reading(self.focus_run_reading(config), SessionFocusTarget::FocusRun);
        let totals = GroupItem::reading(&self.daily_totals, SessionFocusTarget::DailyTotals);
        let switches = GroupItem::reading(
            self.context_switch_rate(config),
            SessionFocusTarget::ContextSwitches,
        );
        Self::expandable_group(
            &self.expansion,
            SessionFocusMessage::Expansion,
            [session, breaks, focus_run, totals, switches],
        )
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: SessionFocusTarget,
        config: &Config,
        clock: Clock,
    ) -> HoverCard {
        let body = match target {
            SessionFocusTarget::Session => self.session_face(config).hover_card(clock),
            SessionFocusTarget::Breaks => self.break_log(config).hover_card(clock),
            SessionFocusTarget::FocusRun => self.focus_run_reading(config).hover_card(clock),
            SessionFocusTarget::DailyTotals => self.daily_totals.hover_card(clock),
            SessionFocusTarget::ContextSwitches => {
                self.context_switch_rate(config).hover_card(clock)
            }
        };
        HoverCard::sourced(body, DeliveryTiming::LivePush)
    }
}
