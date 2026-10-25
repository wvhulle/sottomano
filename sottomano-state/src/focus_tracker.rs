use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use futures::{Stream, StreamExt, stream};
use niri_ipc::state::EventStreamState;
use sottomano_config::widget_groups::focus_time::FocusTimeConfig;
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::kitty_terminal_windows::WindowProcesses,
    login_session_presence::{
        focus_time_ledger::{FocusIdentity, FocusLedger, FocusSignal, LoadedSpans},
        logind_manager,
        wayland_idle_notify::idle_edges,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::focus_spans;

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
    Signal(FocusSignal),
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

    pub fn idle_messages(
        idle_detection_delay: &Duration,
    ) -> impl Stream<Item = TrackerMessage> + Send + use<> {
        idle_edges(*idle_detection_delay)
            .map(|edge| TrackerMessage::Signal(FocusSignal::Idle(edge)))
    }

    pub fn sleep_messages(
        buses: &DesktopBuses,
    ) -> impl Stream<Item = TrackerMessage> + Send + use<> {
        logind_manager::sleep_edges(buses)
            .map(|edge| TrackerMessage::Signal(FocusSignal::Sleep(edge)))
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
            TrackerMessage::Signal(match probed {
                ProbedWindowFocus::FocusedWindowIdentity(identity) => {
                    blocking("focus probe", move || {
                        FocusSignal::WindowFocused(identity.probed())
                    })
                    .await
                }
                ProbedWindowFocus::NoWindowFocused => FocusSignal::NoWindowFocused,
            })
        })
    }

    pub fn update(
        &mut self,
        message: TrackerMessage,
        config: &FocusTimeConfig,
    ) -> impl Future<Output = ()> + Send + use<> {
        let span_writes = match message {
            TrackerMessage::Loaded(loaded) => {
                self.ledger.load_history(loaded, config);
                Vec::new()
            }
            TrackerMessage::Signal(signal) => self.ledger.fold(signal, Utc::now(), config),
        };
        stream::iter(span_writes).for_each(focus_spans::record)
    }
}
