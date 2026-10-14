mod context_switch_rate;
mod daily_totals_gauge;
mod session_face;
mod window_usage;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;

use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        source::{focus_ledger::WakingWindow, niri::OutputInfo, window_agents::WindowProcesses},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    state::focus_tracker::{FocusTracker, TrackerMessage},
    widget_groups::session_focus_group::{
        context_switch_rate::ContextSwitchRate, daily_totals_gauge::DailyTotalsGauge,
        session_face::SessionFace, window_usage::WindowUsage,
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        ongoing_toast::{OngoingToast, OngoingToastActionButton},
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

#[derive(Default)]
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
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn load() -> Task<SessionFocusMessage> {
        FocusTracker::load().map(SessionFocusMessage::Tracker)
    }

    pub fn subscription(
        outputs: &HashMap<String, OutputInfo>,
    ) -> Subscription<SessionFocusMessage> {
        FocusTracker::subscription(outputs).map(SessionFocusMessage::Tracker)
    }

    pub fn refocus(
        &mut self,
        niri: &EventStreamState,
        processes: &WindowProcesses,
    ) -> Task<SessionFocusMessage> {
        self.tracker
            .refocus(niri, processes)
            .map(SessionFocusMessage::Tracker)
    }

    pub fn update(&mut self, message: SessionFocusMessage) -> Task<SessionFocusMessage> {
        match message {
            SessionFocusMessage::Tracker(message) => self
                .tracker
                .update(message)
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
    ) -> DailyTotalsGauge<'a> {
        DailyTotalsGauge {
            accounting: self
                .tracker
                .ledger()
                .accounted(&config.widget_groups.focus_time),
            waking,
        }
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
        let gauge = self.daily_totals_gauge(config, waking);
        let budget_toast = OngoingToast::from_quota_pace(
            SessionFocusTarget::DailyTotals,
            &gauge,
            SessionFocusMessage::Expansion(ExpansionMessage::Expand),
        );
        let totals =
            GroupItem::reading(gauge, SessionFocusTarget::DailyTotals).ongoing_toasts(budget_toast);
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
