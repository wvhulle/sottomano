//! niri integration via the `niri-ipc` event stream.

use std::{fmt, io::Error, thread};

use niri_ipc::{Event, Request, socket::Socket, state::EventStreamState};
use tokio::sync::mpsc;

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

pub fn active_workspace_id(state: &EventStreamState, output: &str) -> Option<u64> {
    state
        .workspaces
        .workspaces
        .values()
        .find(|w| w.output.as_deref() == Some(output) && w.is_active)
        .map(|w| w.id)
}

#[derive(Debug, Clone)]
pub struct WinEntry {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub is_focused: bool,
    pub is_urgent: bool,
}

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

pub fn focus_window(id: u64) {
    if let Err(e) = send_action(niri_ipc::Action::FocusWindow { id }) {
        eprintln!("niri: focus window {id}: {e}");
    }
}

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
