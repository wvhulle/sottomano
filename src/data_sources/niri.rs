//! niri integration via the `niri-ipc` event stream.

use std::{fmt, io::Error, thread};

use niri_ipc::{
    Event, Request, Response,
    socket::Socket,
    state::{EventStreamState, EventStreamStatePart},
};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum OutputsError {
    /// Socket connect or transport failure.
    Transport(Error),
    /// niri understood the request but refused it (its own error message).
    Rejected(String),
    /// niri answered, but not with the `Outputs` variant we asked for.
    Unexpected(Response),
}

impl From<Error> for OutputsError {
    fn from(e: Error) -> Self {
        Self::Transport(e)
    }
}

impl fmt::Display for OutputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "outputs request transport failed: {e}"),
            Self::Rejected(m) => write!(f, "niri rejected outputs request: {m}"),
            Self::Unexpected(r) => write!(f, "unexpected reply to outputs request: {r:?}"),
        }
    }
}

pub fn list_outputs() -> Result<Vec<String>, OutputsError> {
    let mut socket = Socket::connect()?;
    let response = socket
        .send(Request::Outputs)?
        .map_err(OutputsError::Rejected)?;
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

/// The id of the active workspace on an output — the workspace the Windowlist
/// filters its windows by (odin's `NiriService.activeWsId(output)`).
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

/// Windows on a workspace, in niri's on-screen order (column, tile, id) — the
/// order the Windowlist pager lays them out.
pub fn windows_on_workspace(state: &EventStreamState, workspace: u64) -> Vec<WinEntry> {
    let mut windows: Vec<&niri_ipc::Window> = state
        .windows
        .windows
        .values()
        .filter(|w| w.workspace_id == Some(workspace))
        .collect();
    windows.sort_by_key(|w| {
        let (col, tile) = w
            .layout
            .pos_in_scrolling_layout
            .unwrap_or((usize::MAX, usize::MAX));
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
    Transport(Error),
    Rejected(String),
}

impl From<Error> for ActionError {
    fn from(e: Error) -> Self {
        Self::Transport(e)
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "action transport failed: {e}"),
            Self::Rejected(m) => write!(f, "niri rejected action: {m}"),
        }
    }
}

/// Focus a window by id (odin's Windowlist click-to-focus). A one-shot blocking
/// action — call off the async executor. Fire-and-forget: a failure is logged,
/// not surfaced.
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

/// Apply one event to the cached state. `apply` returns the event back when it
/// wasn't consumed by this state part; we ignore that here since
/// `EventStreamState` handles all variants we care about.
pub fn apply(state: &mut EventStreamState, event: Event) {
    let _ = state.apply(event);
}
