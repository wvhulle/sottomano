mod daily_totals_gauge;
mod session_face;

use std::{mem::take, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;

use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        source::{
            focus_ledger::{
                FocusIdentity, FocusLedger, FocusSignal, Focused, Folded, LoadedSpans, WakingWindow,
            },
            idle_notify::{IdleEdge, idle_edges},
            logind::{self, SleepEdge},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::blocking,
    state::focus_spans::{self, Written},
    widget_groups::focus_time_group::{
        daily_totals_gauge::DailyTotalsGauge, session_face::SessionFace,
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        ongoing_toast::{OngoingToast, OngoingToastActionButton},
        tooltip_body::{Freshness, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

const PRESENCE_RESOLUTION: Duration = Duration::from_secs(30);
const LOADED_HISTORY: TimeDelta = TimeDelta::days(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTimeTarget {
    Session,
    DailyTotals,
}

impl From<FocusTimeTarget> for HoverTarget {
    fn from(target: FocusTimeTarget) -> Self {
        HoverTarget::FocusTime(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakPledge {
    pub at: DateTime<Utc>,
}

#[derive(Default)]
pub struct FocusTimeGroup {
    ledger: FocusLedger,
    focused: Option<FocusIdentity>,
    pledge: Option<BreakPledge>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum FocusTimeMessage {
    Loaded(LoadedSpans),
    Focused(Option<Focused>),
    Idle(IdleEdge),
    Sleep(SleepEdge),
    Written(Written),
    BreakPledged,
    Expansion(ExpansionMessage),
}

impl FocusTimeGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn load() -> Task<FocusTimeMessage> {
        Task::perform(
            focus_spans::load_since(Utc::now() - LOADED_HISTORY),
            FocusTimeMessage::Loaded,
        )
    }

    pub fn subscription() -> Subscription<FocusTimeMessage> {
        Subscription::batch([
            Subscription::run(|| idle_edges(PRESENCE_RESOLUTION)).map(FocusTimeMessage::Idle),
            Subscription::run(logind::sleep_edges).map(FocusTimeMessage::Sleep),
        ])
    }

    pub fn refocus(&mut self, niri: &EventStreamState) -> Task<FocusTimeMessage> {
        let identity = FocusIdentity::of(niri);
        if identity == self.focused {
            return Task::none();
        }
        self.focused.clone_from(&identity);
        blocking("focus probe", move || identity.map(FocusIdentity::probed))
            .map(FocusTimeMessage::Focused)
    }

    pub fn update(&mut self, message: FocusTimeMessage) -> Task<FocusTimeMessage> {
        match message {
            FocusTimeMessage::Loaded(loaded) => {
                self.ledger = take(&mut self.ledger).with_history(loaded);
                Task::none()
            }
            FocusTimeMessage::Focused(focused) => self.fold(FocusSignal::Focused(focused)),
            FocusTimeMessage::Idle(edge) => self.fold(FocusSignal::Idle(edge)),
            FocusTimeMessage::Sleep(edge) => self.fold(FocusSignal::Sleep(edge)),
            FocusTimeMessage::Written(Written::OpenedFocus(opened)) => {
                self.ledger = take(&mut self.ledger).with_focus_opened(opened);
                Task::none()
            }
            FocusTimeMessage::Written(Written::OpenedPresence(opened)) => {
                self.ledger = take(&mut self.ledger).with_presence_opened(opened);
                Task::none()
            }
            FocusTimeMessage::Written(Written::Extended | Written::Failed) => Task::none(),
            FocusTimeMessage::BreakPledged => {
                self.pledge = Some(BreakPledge { at: Utc::now() });
                Task::none()
            }
            FocusTimeMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
                Task::none()
            }
        }
    }

    fn fold(&mut self, signal: FocusSignal) -> Task<FocusTimeMessage> {
        let Folded { ledger, writes } = take(&mut self.ledger).fold(signal, Utc::now());
        self.ledger = ledger;
        Task::batch(
            writes
                .into_iter()
                .map(|write| Task::perform(focus_spans::apply(write), FocusTimeMessage::Written)),
        )
    }

    fn session_face<'a>(&'a self, config: &'a Config) -> SessionFace<'a> {
        SessionFace {
            accounting: self.ledger.accounted(&config.widget_groups.focus_time),
            pledge: self.pledge,
        }
    }

    fn daily_totals_gauge<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> DailyTotalsGauge<'a> {
        DailyTotalsGauge {
            accounting: self.ledger.accounted(&config.widget_groups.focus_time),
            waking,
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> GroupContent<'a, FocusTimeMessage> {
        let session = GroupItem::reading(self.session_face(config), FocusTimeTarget::Session)
            .ongoing_toast_action_buttons([OngoingToastActionButton {
                button_label: "Taking a break".to_owned(),
                tint: ActionTint::Proceed,
                on_button_pressed: FocusTimeMessage::BreakPledged,
            }]);
        let gauge = self.daily_totals_gauge(config, waking);
        let budget_toast = OngoingToast::from_quota_pace(
            FocusTimeTarget::DailyTotals,
            &gauge,
            FocusTimeMessage::Expansion(ExpansionMessage::Expand),
        );
        let totals =
            GroupItem::reading(gauge, FocusTimeTarget::DailyTotals).ongoing_toasts(budget_toast);
        GroupContent::new(
            &self.expansion,
            FocusTimeMessage::Expansion,
            [session, totals],
        )
    }

    pub fn tooltip_for(
        &self,
        target: FocusTimeTarget,
        config: &Config,
        waking: WakingWindow,
    ) -> Tooltip {
        let body = match target {
            FocusTimeTarget::Session => self.session_face(config).tooltip(),
            FocusTimeTarget::DailyTotals => self.daily_totals_gauge(config, waking).tooltip(),
        };
        Tooltip::sourced(body, Freshness::Live)
    }
}
