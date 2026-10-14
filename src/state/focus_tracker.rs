use std::{
    collections::{BTreeSet, HashMap},
    mem::take,
    time::Duration,
};

use chrono::{TimeDelta, Utc};
use iced::{Subscription, Task};
use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{
        Delivery, Source,
        drm_connectors::ScreenPower,
        focus_ledger::{FocusIdentity, FocusLedger, FocusSignal, Focused, Folded, LoadedSpans},
        idle_notify::{IdleEdge, idle_edges},
        logind::{self, SleepEdge},
        niri::OutputInfo,
        window_agents::WindowProcesses,
    },
    message_subscriptions::{blocking, polling},
    state::focus_spans::{self, Written},
};

const PRESENCE_RESOLUTION: Duration = Duration::from_secs(30);
const LOADED_HISTORY: TimeDelta = TimeDelta::days(2);

#[derive(Default)]
pub struct FocusTracker {
    ledger: FocusLedger,
    focused: Option<FocusIdentity>,
}

#[derive(Debug, Clone)]
pub enum TrackerMessage {
    Loaded(LoadedSpans),
    Focused(Option<Focused>),
    Idle(IdleEdge),
    Sleep(SleepEdge),
    ScreenPower(Delivery<ScreenPower>),
    Written(Written),
}

impl FocusTracker {
    pub fn ledger(&self) -> &FocusLedger {
        &self.ledger
    }

    pub fn load() -> Task<TrackerMessage> {
        Task::perform(
            focus_spans::load_since(Utc::now() - LOADED_HISTORY),
            TrackerMessage::Loaded,
        )
    }

    pub fn subscription(outputs: &HashMap<String, OutputInfo>) -> Subscription<TrackerMessage> {
        let enabled: BTreeSet<String> = outputs.keys().cloned().collect();
        Subscription::batch([
            Subscription::run(|| idle_edges(PRESENCE_RESOLUTION)).map(TrackerMessage::Idle),
            Subscription::run(logind::sleep_edges).map(TrackerMessage::Sleep),
            polling::<ScreenPower>(enabled, None).map(TrackerMessage::ScreenPower),
        ])
    }

    pub fn refocus(
        &mut self,
        niri: &EventStreamState,
        processes: &WindowProcesses,
    ) -> Task<TrackerMessage> {
        let identity = FocusIdentity::of(niri, processes);
        if identity == self.focused {
            return Task::none();
        }
        self.focused.clone_from(&identity);
        blocking("focus probe", move || identity.map(FocusIdentity::probed))
            .map(TrackerMessage::Focused)
    }

    pub fn update(&mut self, message: TrackerMessage) -> Task<TrackerMessage> {
        match message {
            TrackerMessage::Loaded(loaded) => {
                self.ledger = take(&mut self.ledger).with_history(loaded);
                Task::none()
            }
            TrackerMessage::Focused(focused) => self.fold(FocusSignal::Focused(focused)),
            TrackerMessage::Idle(edge) => self.fold(FocusSignal::Idle(edge)),
            TrackerMessage::Sleep(edge) => self.fold(FocusSignal::Sleep(edge)),
            TrackerMessage::ScreenPower(Delivery::Reading(Ok(power))) => {
                self.fold(FocusSignal::Screen(power.edge()))
            }
            TrackerMessage::ScreenPower(Delivery::Reading(Err(failure))) => {
                tracing::warn!("{}: {failure}", ScreenPower::NAME);
                Task::none()
            }
            TrackerMessage::Written(Written::OpenedFocus(opened)) => {
                self.ledger = take(&mut self.ledger).with_focus_opened(opened);
                Task::none()
            }
            TrackerMessage::Written(Written::OpenedPresence(opened)) => {
                self.ledger = take(&mut self.ledger).with_presence_opened(opened);
                Task::none()
            }
            TrackerMessage::Written(Written::Extended | Written::Failed)
            | TrackerMessage::ScreenPower(Delivery::Absent) => Task::none(),
        }
    }

    fn fold(&mut self, signal: FocusSignal) -> Task<TrackerMessage> {
        let Folded { ledger, writes } = take(&mut self.ledger).fold(signal, Utc::now());
        self.ledger = ledger;
        Task::batch(
            writes
                .into_iter()
                .map(|write| Task::perform(focus_spans::apply(write), TrackerMessage::Written)),
        )
    }
}
