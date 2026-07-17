//! niri integration via the `niri-ipc` event stream.
//!
//! Replaces the ~380-line derivation machinery in Odin's
//! `DataProvider/NiriService.qml`. Instead of mirroring two `QAbstractListModel`s and
//! diffing a JSON signature to suppress churn, `niri-ipc` gives us
//! `EventStreamState`: it delivers the full current state up-front, then deltas, and
//! `EventStreamStatePart::apply` keeps it consistent. Per-output active workspace
//! becomes a trivial filter over `state.workspaces.workspaces`.

use std::thread;

use niri_ipc::socket::Socket;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use niri_ipc::{Event, Request, Response};
use tokio::sync::mpsc;

/// A failed `Request::Outputs` round-trip. Flattens niri's nested reply
/// (`io::Result<Result<Response, _>>`) into one error type so the query reads as a
/// straight `?` chain.
#[derive(Debug)]
pub enum OutputsError {
    /// Socket connect or transport failure.
    Transport(std::io::Error),
    /// niri understood the request but refused it (its own error message).
    Rejected(String),
    /// niri answered, but not with the `Outputs` variant we asked for.
    Unexpected(Response),
}

impl From<std::io::Error> for OutputsError {
    fn from(e: std::io::Error) -> Self {
        Self::Transport(e)
    }
}

impl std::fmt::Display for OutputsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "outputs request transport failed: {e}"),
            Self::Rejected(m) => write!(f, "niri rejected outputs request: {m}"),
            Self::Unexpected(r) => write!(f, "unexpected reply to outputs request: {r:?}"),
        }
    }
}

/// One-shot query of the currently-enabled outputs, by niri name (e.g. `DP-1`).
/// Phase 1 opens one bar per (output, position) at startup; this is the enumeration
/// step (`niri msg outputs`). A one-time blocking request on its own socket — call it
/// off the async executor (`spawn_blocking`).
///
/// Filters to outputs mapped to a logical output; a disabled/unmapped output has
/// `logical == None` and gets no bar. Sorted for a stable surface-open order.
///
/// Hotplug is deliberately NOT handled here: layershellev only auto-creates surfaces
/// on new displays in `StartMode::AllScreens`, and even then with an internal window
/// id we never registered; in the named-output mode we use it silently swallows the
/// new-display event. Output *removal* still arrives as an iced `window::close_events`
/// close (see `app::subscription`). Adding a monitor therefore needs a re-enumeration
/// trigger — left to a later, event-driven pass rather than a poll timer.
pub fn list_outputs() -> Result<Vec<String>, OutputsError> {
    let mut socket = Socket::connect()?;
    let response = socket.send(Request::Outputs)?.map_err(OutputsError::Rejected)?;
    let Response::Outputs(outputs) = response else {
        return Err(OutputsError::Unexpected(response));
    };
    let mut names: Vec<String> = outputs
        .into_iter()
        .filter(|(_, o)| o.logical.is_some())
        .map(|(name, _)| name)
        .collect();
    names.sort();
    Ok(names)
}

/// A workspace pill as the widget wants to render it.
#[derive(Debug, Clone)]
pub struct WsPill {
    pub idx: u8,
    pub name: Option<String>,
    pub is_active: bool,
    pub is_urgent: bool,
}

/// Workspaces on a given output, ordered by on-monitor index. This is the exact
/// question `NiriService.activeWsId(output)` / the Workspaces widget answers.
pub fn workspaces_for_output(state: &EventStreamState, output: &str) -> Vec<WsPill> {
    let mut pills: Vec<WsPill> = state
        .workspaces
        .workspaces
        .values()
        .filter(|w| w.output.as_deref() == Some(output))
        .map(|w| WsPill {
            idx: w.idx,
            name: w.name.clone(),
            is_active: w.is_active,
            is_urgent: w.is_urgent,
        })
        .collect();
    pills.sort_by_key(|p| p.idx);
    pills
}

/// Spawn a dedicated OS thread that opens a second niri socket, requests the event
/// stream, and forwards every `Event` over an mpsc channel. The niri stream read is
/// *blocking* (`read_events` hands back a blocking `FnMut`), so it must not run on the
/// async executor — this thread is the bridge. The returned receiver is consumed by
/// the iced Subscription in `app.rs`.
///
/// On socket error (e.g. niri restart) the thread logs and exits; a full port would
/// reconnect with backoff here.
///
/// Uses a tokio mpsc so the blocking thread can `blocking_send` while the async iced
/// Subscription `recv().await`s — see `app::niri_subscription`.
pub fn spawn_event_thread() -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel(256);
    thread::spawn(move || {
        let mut socket = match Socket::connect() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("niri: connect failed: {e}");
                return;
            }
        };
        if let Err(e) = socket.send(Request::EventStream) {
            eprintln!("niri: EventStream request failed: {e}");
            return;
        }
        let mut read = socket.read_events();
        loop {
            match read() {
                Ok(event) => {
                    if tx.blocking_send(event).is_err() {
                        return; // receiver dropped: app is shutting down
                    }
                }
                Err(e) => {
                    eprintln!("niri: event read ended: {e}");
                    return;
                }
            }
        }
    });
    rx
}

/// Apply one event to the cached state. `apply` returns the event back when it wasn't
/// consumed by this state part; we ignore that here since `EventStreamState` handles
/// all variants we care about.
pub fn apply(state: &mut EventStreamState, event: Event) {
    let _ = state.apply(event);
}
