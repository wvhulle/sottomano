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
    pub id: u64,
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
            id: w.id,
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
                    if matches!(event, Event::WindowFocusChanged { .. }) {
                        eprintln!(
                            "niri: thread got WindowFocusChanged at {:?}",
                            std::time::SystemTime::now()
                        );
                    }
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

/// The id of the active workspace on an output — the workspace the Windowlist filters
/// its windows by (odin's `NiriService.activeWsId(output)`).
pub fn active_workspace_id(state: &EventStreamState, output: &str) -> Option<u64> {
    state
        .workspaces
        .workspaces
        .values()
        .find(|w| w.output.as_deref() == Some(output) && w.is_active)
        .map(|w| w.id)
}

/// One window as the Windowlist renders it.
#[derive(Debug, Clone)]
pub struct WinEntry {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub is_focused: bool,
    pub is_urgent: bool,
}

/// Windows on a workspace, in niri's on-screen order (column, tile, id) — the order the
/// Windowlist pager lays them out.
pub fn windows_on_workspace(state: &EventStreamState, workspace: u64) -> Vec<WinEntry> {
    let mut windows: Vec<&niri_ipc::Window> = state
        .windows
        .windows
        .values()
        .filter(|w| w.workspace_id == Some(workspace))
        .collect();
    windows.sort_by_key(|w| {
        let (col, tile) = w.layout.pos_in_scrolling_layout.unwrap_or((usize::MAX, usize::MAX));
        (col, tile, w.id)
    });
    windows
        .into_iter()
        .map(|w| WinEntry {
            id: w.id,
            title: w.title.clone().unwrap_or_default(),
            app_id: w.app_id.clone().unwrap_or_default(),
            is_focused: w.is_focused,
            is_urgent: w.is_urgent,
        })
        .collect()
}

/// A failed niri action round-trip — the write half of the socket, mirroring
/// [`OutputsError`] for the read half.
#[derive(Debug)]
pub enum ActionError {
    Transport(std::io::Error),
    Rejected(String),
}

impl From<std::io::Error> for ActionError {
    fn from(e: std::io::Error) -> Self {
        Self::Transport(e)
    }
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "action transport failed: {e}"),
            Self::Rejected(m) => write!(f, "niri rejected action: {m}"),
        }
    }
}

/// Focus a window by id (odin's Windowlist click-to-focus). A one-shot blocking action —
/// call off the async executor. Fire-and-forget: a failure is logged, not surfaced.
pub fn focus_window(id: u64) {
    if let Err(e) = send_action(niri_ipc::Action::FocusWindow { id }) {
        eprintln!("niri: focus window {id}: {e}");
    }
}

/// Focus a workspace by id (odin's Workspaces click-to-focus).
pub fn focus_workspace(id: u64) {
    let action = niri_ipc::Action::FocusWorkspace {
        reference: niri_ipc::WorkspaceReferenceArg::Id(id),
    };
    if let Err(e) = send_action(action) {
        eprintln!("niri: focus workspace {id}: {e}");
    }
}

fn send_action(action: niri_ipc::Action) -> Result<(), ActionError> {
    let reply = Socket::connect()?.send(Request::Action(action))?;
    reply.map_err(ActionError::Rejected)?;
    Ok(())
}

/// Apply one event to the cached state. `apply` returns the event back when it wasn't
/// consumed by this state part; we ignore that here since `EventStreamState` handles
/// all variants we care about.
pub fn apply(state: &mut EventStreamState, event: Event) {
    let _ = state.apply(event);
}
