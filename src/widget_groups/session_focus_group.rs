mod account_face;
mod breaks_today;
mod context_switch_rate;
mod daily_totals_gauge;
mod longest_run;
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
            Delivery,
            focus_ledger::{
                FocusIdentity, FocusLedger, FocusSignal, Focused, Folded, LoadedSpans, WakingWindow,
            },
            idle_notify::{IdleEdge, idle_edges},
            logind::{
                self, SleepEdge,
                session_facts::{self, SessionFacts, SessionIdentity},
            },
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::blocking,
    state::focus_spans::{self, Written},
    widget_groups::session_focus_group::{
        account_face::AccountFace, breaks_today::BreaksToday,
        context_switch_rate::ContextSwitchRate, daily_totals_gauge::DailyTotalsGauge,
        longest_run::LongestRun, session_face::SessionFace,
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        ongoing_toast::{OngoingToast, OngoingToastActionButton},
        tooltip_body::{Freshness, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

const PRESENCE_RESOLUTION: Duration = Duration::from_secs(30);
const LOADED_HISTORY: TimeDelta = TimeDelta::days(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionFocusTarget {
    Account,
    Session,
    DailyTotals,
    Breaks,
    LongestRun,
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
    account_face: AccountFace,
    account_facts: OnHoverReading<SessionFacts>,
    ledger: FocusLedger,
    focused: Option<FocusIdentity>,
    pledge: Option<BreakPledge>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum SessionFocusMessage {
    IdentityLoaded(Option<SessionIdentity>),
    AccountFactsFetched(Box<Delivery<SessionFacts>>),
    Loaded(LoadedSpans),
    Focused(Option<Focused>),
    Idle(IdleEdge),
    Sleep(SleepEdge),
    Written(Written),
    BreakPledged,
    Expansion(ExpansionMessage),
}

impl SessionFocusGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn load() -> Task<SessionFocusMessage> {
        Task::batch([
            Task::perform(
                focus_spans::load_since(Utc::now() - LOADED_HISTORY),
                SessionFocusMessage::Loaded,
            ),
            blocking("session identity", session_facts::identity)
                .map(SessionFocusMessage::IdentityLoaded),
        ])
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<SessionFocusMessage> {
        self.account_facts.stop();
        match hovered {
            Some(HoverTarget::SessionFocus(SessionFocusTarget::Account)) => {
                self.account_facts.poll(
                    (),
                    |delivery| SessionFocusMessage::AccountFactsFetched(Box::new(delivery)),
                    config,
                )
            }
            _ => Task::none(),
        }
    }

    pub fn subscription() -> Subscription<SessionFocusMessage> {
        Subscription::batch([
            Subscription::run(|| idle_edges(PRESENCE_RESOLUTION)).map(SessionFocusMessage::Idle),
            Subscription::run(logind::sleep_edges).map(SessionFocusMessage::Sleep),
        ])
    }

    pub fn refocus(&mut self, niri: &EventStreamState) -> Task<SessionFocusMessage> {
        let identity = FocusIdentity::of(niri);
        if identity == self.focused {
            return Task::none();
        }
        self.focused.clone_from(&identity);
        blocking("focus probe", move || identity.map(FocusIdentity::probed))
            .map(SessionFocusMessage::Focused)
    }

    pub fn update(&mut self, message: SessionFocusMessage) -> Task<SessionFocusMessage> {
        match message {
            SessionFocusMessage::IdentityLoaded(identity) => {
                self.account_face.adopt(identity);
                Task::none()
            }
            SessionFocusMessage::AccountFactsFetched(delivery) => {
                self.account_facts.receive(*delivery);
                Task::none()
            }
            SessionFocusMessage::Loaded(loaded) => {
                self.ledger = take(&mut self.ledger).with_history(loaded);
                Task::none()
            }
            SessionFocusMessage::Focused(focused) => self.fold(FocusSignal::Focused(focused)),
            SessionFocusMessage::Idle(edge) => self.fold(FocusSignal::Idle(edge)),
            SessionFocusMessage::Sleep(edge) => self.fold(FocusSignal::Sleep(edge)),
            SessionFocusMessage::Written(Written::OpenedFocus(opened)) => {
                self.ledger = take(&mut self.ledger).with_focus_opened(opened);
                Task::none()
            }
            SessionFocusMessage::Written(Written::OpenedPresence(opened)) => {
                self.ledger = take(&mut self.ledger).with_presence_opened(opened);
                Task::none()
            }
            SessionFocusMessage::Written(Written::Extended | Written::Failed) => Task::none(),
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

    fn fold(&mut self, signal: FocusSignal) -> Task<SessionFocusMessage> {
        let Folded { ledger, writes } = take(&mut self.ledger).fold(signal, Utc::now());
        self.ledger = ledger;
        Task::batch(
            writes.into_iter().map(|write| {
                Task::perform(focus_spans::apply(write), SessionFocusMessage::Written)
            }),
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

    fn breaks_today<'a>(&'a self, config: &'a Config, waking: WakingWindow) -> BreaksToday<'a> {
        BreaksToday {
            accounting: self.ledger.accounted(&config.widget_groups.focus_time),
            waking,
        }
    }

    fn longest_run<'a>(&'a self, config: &'a Config, waking: WakingWindow) -> LongestRun<'a> {
        LongestRun {
            accounting: self.ledger.accounted(&config.widget_groups.focus_time),
            waking,
        }
    }

    fn context_switch_rate<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> ContextSwitchRate<'a> {
        ContextSwitchRate {
            accounting: self.ledger.accounted(&config.widget_groups.focus_time),
            waking,
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        waking: WakingWindow,
    ) -> GroupContent<'a, SessionFocusMessage> {
        let session = GroupItem::reading(self.session_face(config), SessionFocusTarget::Session)
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
        let account = self
            .account_face
            .identity()
            .map(|_| GroupItem::reading(&self.account_face, SessionFocusTarget::Account));
        let breaks = GroupItem::reading(
            self.breaks_today(config, waking),
            SessionFocusTarget::Breaks,
        );
        let longest = GroupItem::reading(
            self.longest_run(config, waking),
            SessionFocusTarget::LongestRun,
        );
        let switches = GroupItem::reading(
            self.context_switch_rate(config, waking),
            SessionFocusTarget::ContextSwitches,
        );
        GroupContent::new(
            &self.expansion,
            SessionFocusMessage::Expansion,
            account
                .into_iter()
                .chain([session, totals, breaks, longest, switches]),
        )
    }

    pub fn tooltip_for(
        &self,
        target: SessionFocusTarget,
        config: &Config,
        waking: WakingWindow,
    ) -> Tooltip {
        let body = match target {
            SessionFocusTarget::Account => {
                return Tooltip::sourced(self.account_face.tooltip(), Freshness::Live)
                    .with_sections(self.account_facts.sections(SessionFacts::sections));
            }
            SessionFocusTarget::Session => self.session_face(config).tooltip(),
            SessionFocusTarget::DailyTotals => self.daily_totals_gauge(config, waking).tooltip(),
            SessionFocusTarget::Breaks => self.breaks_today(config, waking).tooltip(),
            SessionFocusTarget::LongestRun => self.longest_run(config, waking).tooltip(),
            SessionFocusTarget::ContextSwitches => {
                self.context_switch_rate(config, waking).tooltip()
            }
        };
        Tooltip::sourced(body, Freshness::Live)
    }
}
