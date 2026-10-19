use std::{mem::replace, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use futures::{Stream, StreamExt, stream};
use niri_ipc::state::EventStreamState;
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::kitty_terminal_windows::WindowProcesses,
    login_session_presence::{
        focus_time_ledger::{
            AwayReasonEdge, FocusIdentity, FocusLedger, FocusSignal, Folded, LoadedSpans,
            ScreenEdge, SpanWrite, WakingWindow, WindowFocusChange,
        },
        logind_manager::{self, SleepEdge},
        wayland_idle_notify::{IdleEdge, idle_edges},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::focus_spans;

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
    #[must_use]
    pub fn started_at(now: DateTime<Utc>) -> Self {
        Self {
            ledger: FocusLedger::started_at(now),
            last_probed_window_focus: ProbedWindowFocus::NoWindowFocused,
        }
    }

    #[must_use]
    pub fn ledger(&self) -> &FocusLedger {
        &self.ledger
    }

    pub async fn load() -> TrackerMessage {
        TrackerMessage::Loaded(focus_spans::load_since(Utc::now() - LOADED_HISTORY).await)
    }

    pub fn idle_messages() -> impl Stream<Item = TrackerMessage> + Send {
        idle_edges(PRESENCE_RESOLUTION).map(TrackerMessage::Idle)
    }

    pub fn sleep_messages(
        buses: &DesktopBuses,
    ) -> impl Stream<Item = TrackerMessage> + Send + use<> {
        logind_manager::sleep_edges(buses).map(TrackerMessage::Sleep)
    }

    pub fn refocus(
        &mut self,
        niri: &EventStreamState,
        processes: &WindowProcesses,
    ) -> Option<impl Future<Output = TrackerMessage> + Send + use<>> {
        let probed = FocusIdentity::of(niri, processes).map_or(
            ProbedWindowFocus::NoWindowFocused,
            ProbedWindowFocus::FocusedWindowIdentity,
        );
        if probed == self.last_probed_window_focus {
            return None;
        }
        self.last_probed_window_focus = probed.clone();
        Some(async move {
            TrackerMessage::WindowFocusChanged(match probed {
                ProbedWindowFocus::FocusedWindowIdentity(identity) => {
                    blocking("focus probe", move || {
                        WindowFocusChange::WindowGainedFocus(identity.probed())
                    })
                    .await
                }
                ProbedWindowFocus::NoWindowFocused => WindowFocusChange::NoWindowFocused,
            })
        })
    }

    pub fn update(
        &mut self,
        message: TrackerMessage,
        waking: WakingWindow,
    ) -> impl Future<Output = ()> + Send + use<> {
        let now = Utc::now();
        let span_writes = match message {
            TrackerMessage::Loaded(loaded) => {
                self.ledger = self.taken_ledger().with_history(loaded);
                Vec::new()
            }
            TrackerMessage::WindowFocusChanged(change) => {
                self.fold(FocusSignal::WindowFocusChanged(change), now)
            }
            TrackerMessage::Idle(edge) => self.fold(
                FocusSignal::AwayReasonChanged(AwayReasonEdge::IdleNotifier(edge)),
                now,
            ),
            TrackerMessage::Sleep(edge) => self.fold(
                FocusSignal::AwayReasonChanged(AwayReasonEdge::LogindSleep(edge)),
                now,
            ),
            TrackerMessage::ScreenPowerChanged(edge) => self.fold(
                FocusSignal::AwayReasonChanged(AwayReasonEdge::ScreenPower(edge)),
                now,
            ),
        };
        self.ledger = self
            .taken_ledger()
            .with_settled_unit_totals(waking.around(now), now);
        stream::iter(span_writes).for_each(focus_spans::record)
    }

    fn taken_ledger(&mut self) -> FocusLedger {
        replace(&mut self.ledger, FocusLedger::started_at(Utc::now()))
    }

    fn fold(&mut self, signal: FocusSignal, now: DateTime<Utc>) -> Vec<SpanWrite> {
        let Folded {
            ledger,
            span_writes,
        } = self.taken_ledger().fold(signal, now);
        self.ledger = ledger;
        span_writes
    }
}
