//! niri integration via the `niri-ipc` event stream.

use std::{collections::HashMap, fmt, io::Error, thread};

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

/// The bar-facing derivation of [`EventStreamState`]: workspaces grouped and
/// sorted per output, windows grouped and sorted per workspace. Recomputed once
/// per niri event (see `Bar::apply_niri_event`), so `view` reads borrowed
/// slices instead of re-collecting and re-cloning on every redraw.
#[derive(Default)]
pub struct Snapshot {
    workspaces: HashMap<String, Vec<WsPill>>,
    windows: HashMap<u64, Vec<WinEntry>>,
    active: HashMap<String, u64>,
}

impl Snapshot {
    pub fn derive(state: &EventStreamState) -> Self {
        let mut workspaces: HashMap<String, Vec<WsPill>> = HashMap::new();
        let mut active = HashMap::new();
        for w in state.workspaces.workspaces.values() {
            let Some(output) = &w.output else { continue };
            if w.is_active {
                active.insert(output.clone(), w.id);
            }
            workspaces.entry(output.clone()).or_default().push(WsPill {
                id: w.id,
                idx: w.idx,
                name: w.name.clone(),
                is_active: w.is_active,
                is_urgent: w.is_urgent,
            });
        }
        for pills in workspaces.values_mut() {
            pills.sort_by_key(|p| p.idx);
        }

        let mut grouped: HashMap<u64, Vec<&niri_ipc::Window>> = HashMap::new();
        for w in state.windows.windows.values() {
            let Some(workspace) = w.workspace_id else {
                continue;
            };
            grouped.entry(workspace).or_default().push(w);
        }
        let windows = grouped
            .into_iter()
            .map(|(workspace, mut group)| {
                group.sort_by_key(|w| {
                    let (col, tile) = w
                        .layout
                        .pos_in_scrolling_layout
                        .unwrap_or((usize::MAX, usize::MAX));
                    (col, tile, w.id)
                });
                (workspace, group.into_iter().map(win_entry).collect())
            })
            .collect();

        Self {
            workspaces,
            windows,
            active,
        }
    }

    /// The output's workspaces, sorted by index.
    pub fn workspaces_for_output(&self, output: &str) -> &[WsPill] {
        self.workspaces.get(output).map_or(&[], Vec::as_slice)
    }

    /// The workspace's windows, in scrolling-layout order.
    pub fn windows_on_workspace(&self, workspace: u64) -> &[WinEntry] {
        self.windows.get(&workspace).map_or(&[], Vec::as_slice)
    }

    pub fn active_workspace_id(&self, output: &str) -> Option<u64> {
        self.active.get(output).copied()
    }
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

#[derive(Debug, Clone)]
pub struct WinEntry {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub is_focused: bool,
    pub is_urgent: bool,
}

/// Every window niri currently knows about, regardless of workspace/output —
/// used to gather icon keys that need resolving across the whole bar, not
/// just the windows shown on one output's workspace.
pub fn all_windows(state: &EventStreamState) -> Vec<WinEntry> {
    state.windows.windows.values().map(win_entry).collect()
}

fn win_entry(w: &niri_ipc::Window) -> WinEntry {
    WinEntry {
        id: w.id,
        title: w.title.clone().unwrap_or_default(),
        app_id: w.app_id.clone().unwrap_or_default(),
        is_focused: w.is_focused,
        is_urgent: w.is_urgent,
    }
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
