mod context_switch_rate;
mod daily_totals_gauge;
mod session_face;
mod window_usage;

use chrono::{DateTime, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;
use vetrata_config::Config;
use vetrata_sources::{
    bus::DesktopBuses,
    focus_ledger::{ScreenEdge, WakingWindow},
    kitty_windows::WindowProcesses,
    notifications::model::ActionTint,
};
use vetrata_state::focus_tracker::{FocusTracker, TrackerMessage};

use crate::{
    hover_tooltips::HoverTarget,
    widget_groups::session_focus_group::{
        context_switch_rate::ContextSwitchRate, daily_totals_gauge::DailyTotalsGauge,
        session_face::SessionFace, window_usage::WindowUsage,
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
}

#[derive(Debug, Clone)]
pub enum SessionFocusMessage {
    Tracker(TrackerMessage),
    BreakPledged,
    Expansion(ExpansionMessage),
}

impl SessionFocusGroup {
    #[must_use]
    pub fn started_at(now: DateTime<Utc>) -> Self {
        Self {
            tracker: FocusTracker::started_at(now),
            pledge: None,
            expansion: GroupExpansion::default(),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn load() -> Task<SessionFocusMessage> {
        FocusTracker::load().map(SessionFocusMessage::Tracker)
    }

    pub fn subscription() -> Subscription<SessionFocusMessage> {
        FocusTracker::subscription().map(SessionFocusMessage::Tracker)
    }

    pub fn sleep_subscription(buses: &DesktopBuses) -> Subscription<SessionFocusMessage> {
        FocusTracker::sleep_subscription(buses).map(SessionFocusMessage::Tracker)
    }

    pub fn screen_power_changed(
        &mut self,
        edge: ScreenEdge,
        waking: WakingWindow,
    ) -> Task<SessionFocusMessage> {
        self.update(
            SessionFocusMessage::Tracker(TrackerMessage::ScreenPowerChanged(edge)),
            waking,
        )
    }

    pub fn refocus(
        &mut self,
        niri: &EventStreamState,
        processes: &WindowProcesses,
        waking: WakingWindow,
    ) -> Task<SessionFocusMessage> {
        self.tracker
            .refocus(niri, processes, waking)
            .map(SessionFocusMessage::Tracker)
    }

    pub fn update(
        &mut self,
        message: SessionFocusMessage,
        waking: WakingWindow,
    ) -> Task<SessionFocusMessage> {
        match message {
            SessionFocusMessage::Tracker(message) => self
                .tracker
                .update(message, waking)
                .map(SessionFocusMessage::Tracker),
            SessionFocusMessage::BreakPledged => {
                self.pledge = Some(BreakPledge { at: Utc::now() });
                Task::none()
            }
            SessionFocusMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
                Task::none()
            }
        }
    }

    fn session_face<'a>(&'a self, config: &'a Config, waking: WakingWindow) -> SessionFace<'a> {
        SessionFace {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            waking,
            pledge: self.pledge,
        }
    }

    fn daily_totals_gauge<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> DailyTotalsGauge {
        DailyTotalsGauge::new(
            &self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            waking,
        )
    }

    fn context_switch_rate<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> ContextSwitchRate<'a> {
        ContextSwitchRate {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            waking,
        }
    }

    #[must_use]
    pub fn window_usage_tooltip(
        &self,
        window: u64,
        config: &Config,
        waking: WakingWindow,
    ) -> Tooltip {
        let usage = WindowUsage {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            waking,
            window,
        };
        Tooltip::sourced(usage.body(), Freshness::Live)
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> GroupContent<'a, SessionFocusMessage> {
        let session = GroupItem::reading(
            self.session_face(config, waking),
            SessionFocusTarget::Session,
        )
        .ongoing_toast_action_buttons([OngoingToastActionButton {
            button_label: "Taking a break".to_owned(),
            tint: ActionTint::Proceed,
            on_button_pressed: SessionFocusMessage::BreakPledged,
        }]);
        let totals = GroupItem::reading(
            self.daily_totals_gauge(config, waking),
            SessionFocusTarget::DailyTotals,
        );
        let switches = GroupItem::reading(
            self.context_switch_rate(config, waking),
            SessionFocusTarget::ContextSwitches,
        );
        GroupContent::new(
            &self.expansion,
            SessionFocusMessage::Expansion,
            [session, totals, switches],
        )
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: SessionFocusTarget,
        config: &Config,
        waking: WakingWindow,
    ) -> Tooltip {
        let body = match target {
            SessionFocusTarget::Session => self.session_face(config, waking).tooltip(),
            SessionFocusTarget::DailyTotals => self.daily_totals_gauge(config, waking).tooltip(),
            SessionFocusTarget::ContextSwitches => {
                self.context_switch_rate(config, waking).tooltip()
            }
        };
        Tooltip::sourced(body, Freshness::Live)
    }
}
