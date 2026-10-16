use std::{mem::replace, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{
        focus_ledger::{
            AwayReasonEdge, FocusIdentity, FocusLedger, FocusSignal, Folded, LoadedSpans,
            ScreenEdge, WindowFocusChange,
        },
        idle_notify::{IdleEdge, idle_edges},
        logind::{self, SleepEdge},
        window_agents::WindowProcesses,
    },
    message_subscriptions::blocking,
    state::focus_spans,
};

const PRESENCE_RESOLUTION: Duration = Duration::from_secs(30);
const LOADED_HISTORY: TimeDelta = TimeDelta::days(1);

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProbedWindowFocus {
    FocusedWindowIdentity(FocusIdentity),
    NoWindowFocused,
}

pub struct FocusTracker {
    ledger: FocusLedger,
    last_probed_window_focus: ProbedWindowFocus,
}

#[derive(Debug, Clone)]
pub enum TrackerMessage {
    Loaded(LoadedSpans),
    WindowFocusChanged(WindowFocusChange),
    Idle(IdleEdge),
    Sleep(SleepEdge),
    ScreenPowerChanged(ScreenEdge),
}

impl FocusTracker {
    pub fn started_at(now: DateTime<Utc>) -> Self {
        Self {
            ledger: FocusLedger::started_at(now),
            last_probed_window_focus: ProbedWindowFocus::NoWindowFocused,
        }
    }

    pub fn ledger(&self) -> &FocusLedger {
        &self.ledger
    }

    pub fn load() -> Task<TrackerMessage> {
        Task::perform(
            focus_spans::load_since(Utc::now() - LOADED_HISTORY),
            TrackerMessage::Loaded,
        )
    }

    pub fn subscription() -> Subscription<TrackerMessage> {
        Subscription::batch([
            Subscription::run(|| idle_edges(PRESENCE_RESOLUTION)).map(TrackerMessage::Idle),
            Subscription::run(logind::sleep_edges).map(TrackerMessage::Sleep),
        ])
    }

    pub fn refocus(
        &mut self,
        niri: &EventStreamState,
        processes: &WindowProcesses,
    ) -> Task<TrackerMessage> {
        let probed = FocusIdentity::of(niri, processes).map_or(
            ProbedWindowFocus::NoWindowFocused,
            ProbedWindowFocus::FocusedWindowIdentity,
        );
        if probed == self.last_probed_window_focus {
            return Task::none();
        }
        self.last_probed_window_focus = probed.clone();
        match probed {
            ProbedWindowFocus::FocusedWindowIdentity(identity) => {
                blocking("focus probe", move || {
                    WindowFocusChange::WindowGainedFocus(identity.probed())
                })
                .map(TrackerMessage::WindowFocusChanged)
            }
            ProbedWindowFocus::NoWindowFocused => self.fold(FocusSignal::WindowFocusChanged(
                WindowFocusChange::NoWindowFocused,
            )),
        }
    }

    pub fn update(&mut self, message: TrackerMessage) -> Task<TrackerMessage> {
        match message {
            TrackerMessage::Loaded(loaded) => {
                self.ledger = self.taken_ledger().with_history(loaded);
                Task::none()
            }
            TrackerMessage::WindowFocusChanged(change) => {
                self.fold(FocusSignal::WindowFocusChanged(change))
            }
            TrackerMessage::Idle(edge) => {
                self.fold_away_reason(AwayReasonEdge::IdleNotifierEdge(edge))
            }
            TrackerMessage::Sleep(edge) => {
                self.fold_away_reason(AwayReasonEdge::LogindSleepEdge(edge))
            }
            TrackerMessage::ScreenPowerChanged(edge) => {
                self.fold_away_reason(AwayReasonEdge::ScreenPowerEdge(edge))
            }
        }
    }

    fn fold_away_reason(&mut self, edge: AwayReasonEdge) -> Task<TrackerMessage> {
        self.fold(FocusSignal::AwayReasonChanged(edge))
    }

    fn taken_ledger(&mut self) -> FocusLedger {
        replace(&mut self.ledger, FocusLedger::started_at(Utc::now()))
    }

    fn fold(&mut self, signal: FocusSignal) -> Task<TrackerMessage> {
        let Folded {
            ledger,
            span_writes,
        } = self.taken_ledger().fold(signal, Utc::now());
        self.ledger = ledger;
        Task::batch(
            span_writes
                .into_iter()
                .map(|write| Task::future(focus_spans::record(write)).discard()),
        )
    }
}
