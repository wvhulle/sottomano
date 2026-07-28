//! niri integration via the `niri-ipc` event stream.

use std::{collections::HashMap, fmt, io::Error, thread};

use niri_ipc::{Event, Request, Response, socket::Socket, state::EventStreamState};
use tokio::sync::mpsc;

/// Vertical half of a screen position: a notification toast anchors to the top
/// or bottom edge, or sits vertically centered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VEdge {
    Top,
    Center,
    Bottom,
}

/// Horizontal half of a screen position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HEdge {
    Left,
    Center,
    Right,
}

/// Where a window sits on its own output, relative to the active workspace
/// (vertical) and its workspace's focused column (horizontal) — the signal a
/// notification toast points back toward its producer with. `output` is `None`
/// for a window niri no longer places.
#[derive(Debug, Clone)]
pub struct WindowEdge {
    pub vertical: VEdge,
    pub horizontal: HEdge,
    pub output: Option<String>,
}

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
/// per coalesced niri-event burst (see `Bar::apply_niri_events`), so `view`
/// reads borrowed slices instead of re-collecting and re-cloning on every
/// redraw.
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

    /// The name of the output's active workspace, if it has one. Quick-launch
    /// buckets are keyed by workspace name, so an unnamed active workspace has
    /// no bucket (and thus no quick-launch offerings).
    pub fn active_workspace_name(&self, output: &str) -> Option<&str> {
        let id = self.active_workspace_id(output)?;
        self.workspaces
            .get(output)?
            .iter()
            .find(|pill| pill.id == id)
            .and_then(|pill| pill.name.as_deref())
    }

    /// Distinct, non-empty app-ids of the windows open on the output's active
    /// workspace — the set quick-launch excludes as already-open.
    pub fn open_app_ids(&self, output: &str) -> Vec<String> {
        let Some(id) = self.active_workspace_id(output) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = self
            .windows_on_workspace(id)
            .iter()
            .map(|win| win.app_id.clone())
            .filter(|app_id| !app_id.is_empty())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
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

/// Notification-placement facts derived from niri per event: which workspace
/// (and output) each window is on, the globally focused window, and each
/// window's edge on its own output. Ports odin's `NiriService._recompute`.
///
/// Unlike odin — which had to remember each workspace's focused window in a
/// stateful map because its Quickshell niri plugin exposed no per-workspace
/// active window — this reads `Workspace::active_window_id` directly, so the
/// derivation is a pure function of the event-stream state.
#[derive(Default)]
pub struct Placement {
    workspace_by_window: HashMap<u64, u64>,
    focused_window: Option<u64>,
    output_by_workspace: HashMap<u64, String>,
    edges: HashMap<u64, WindowEdge>,
    /// Live windows' identities, for resolving a notification to its source
    /// window by app-id / title when it carries no `x-niri-window-id` hint.
    windows: Vec<WindowIdentity>,
}

struct WindowIdentity {
    id: u64,
    app_id: Option<String>,
    title: Option<String>,
}

impl Placement {
    pub fn derive(state: &EventStreamState) -> Self {
        let mut ws_idx = HashMap::new();
        let mut output_by_workspace = HashMap::new();
        let mut active_ws_by_output: HashMap<String, u64> = HashMap::new();
        let mut ws_active_window = HashMap::new();
        for w in state.workspaces.workspaces.values() {
            ws_idx.insert(w.id, w.idx);
            if let Some(output) = &w.output {
                output_by_workspace.insert(w.id, output.clone());
                if w.is_active {
                    active_ws_by_output.insert(output.clone(), w.id);
                }
            }
            if let Some(active) = w.active_window_id {
                ws_active_window.insert(w.id, active);
            }
        }

        let mut workspace_by_window = HashMap::new();
        let mut column_by_window = HashMap::new();
        let mut focused_window = None;
        let mut windows = Vec::new();
        for w in state.windows.windows.values() {
            if let Some(ws) = w.workspace_id {
                workspace_by_window.insert(w.id, ws);
            }
            if let Some((col, _)) = w.layout.pos_in_scrolling_layout {
                column_by_window.insert(w.id, col);
            }
            if w.is_focused {
                focused_window = Some(w.id);
            }
            windows.push(WindowIdentity {
                id: w.id,
                app_id: w.app_id.clone(),
                title: w.title.clone(),
            });
        }

        let edges = workspace_by_window
            .iter()
            .map(|(&id, &ws)| {
                let output = output_by_workspace.get(&ws).cloned();
                let active_ws = output
                    .as_ref()
                    .and_then(|o| active_ws_by_output.get(o).copied());
                let vertical = vertical_edge(
                    ws_idx.get(&ws).copied(),
                    active_ws.and_then(|a| ws_idx.get(&a).copied()),
                );
                // Left/right only for a window on its output's visible (active)
                // workspace; a background workspace points via the vertical axis
                // alone.
                let horizontal = if active_ws == Some(ws) {
                    let ws_focused_col = ws_active_window
                        .get(&ws)
                        .and_then(|win| column_by_window.get(win).copied());
                    horizontal_edge(column_by_window.get(&id).copied(), ws_focused_col)
                } else {
                    HEdge::Center
                };
                (
                    id,
                    WindowEdge {
                        vertical,
                        horizontal,
                        output,
                    },
                )
            })
            .collect();

        Self {
            workspace_by_window,
            focused_window,
            output_by_workspace,
            edges,
            windows,
        }
    }

    pub fn window_edge(&self, id: u64) -> WindowEdge {
        self.edges.get(&id).cloned().unwrap_or(WindowEdge {
            vertical: VEdge::Center,
            horizontal: HEdge::Center,
            output: None,
        })
    }

    pub fn focused_window(&self) -> Option<u64> {
        self.focused_window
    }

    /// The workspace a window is on, or `None` when niri no longer places it —
    /// the signal a notification's target window has closed.
    pub fn workspace_of_window(&self, id: u64) -> Option<u64> {
        self.workspace_by_window.get(&id).copied()
    }

    pub fn output_of_workspace(&self, ws: u64) -> Option<&str> {
        self.output_by_workspace.get(&ws).map(String::as_str)
    }

    /// The workspace to move a window "here" to: the one the focused window is
    /// on (odin's `workspaceByWindowId[focusedWindowId]`).
    pub fn active_workspace_for_move(&self) -> Option<u64> {
        self.workspace_by_window.get(&self.focused_window?).copied()
    }
}

/// Producer's workspace index vs the active workspace index on its own output.
fn vertical_edge(window_ws_idx: Option<u8>, active_ws_idx: Option<u8>) -> VEdge {
    match (window_ws_idx, active_ws_idx) {
        (Some(w), Some(a)) if w < a => VEdge::Top,
        (Some(w), Some(a)) if w > a => VEdge::Bottom,
        _ => VEdge::Center,
    }
}

/// Producer's column vs its workspace's focused column.
fn horizontal_edge(window_col: Option<usize>, ws_focused_col: Option<usize>) -> HEdge {
    match (window_col, ws_focused_col) {
        (Some(w), Some(f)) if w < f => HEdge::Left,
        (Some(w), Some(f)) if w > f => HEdge::Right,
        _ => HEdge::Center,
    }
}

/// A unique window matching the given source identifiers: exact
/// (case-insensitive) app-id match first, then substring title match. Returns
/// `None` when zero or multiple windows match at each stage — ambiguity is
/// treated the same as absence. Ports odin's `NiriService.windowIdBySource`.
impl Placement {
    /// The window a source matches: a unique live window whose app-id is one of
    /// `app_ids`, else a unique one whose title contains `title`. `None` on
    /// zero or ambiguous matches. Used to resolve a notification with no
    /// `x-niri-window-id` hint back to its producer window.
    pub fn window_by_source(&self, app_ids: &[String], title: Option<&str>) -> Option<u64> {
        let app_id_needles: Vec<String> = app_ids
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect();
        let mut app_match = self.windows.iter().filter(|w| {
            w.app_id
                .as_ref()
                .map(|a| a.to_lowercase())
                .is_some_and(|a| !a.is_empty() && app_id_needles.contains(&a))
        });
        if let Some(only) = unique(&mut app_match) {
            return Some(only.id);
        }

        let needle = title?.to_lowercase();
        let mut title_match = self.windows.iter().filter(|w| {
            w.title
                .as_ref()
                .is_some_and(|t| !t.is_empty() && t.to_lowercase().contains(&needle))
        });
        unique(&mut title_match).map(|w| w.id)
    }
}

/// The single element of an iterator, or `None` if it has zero or more than
/// one.
fn unique<T>(iter: &mut impl Iterator<Item = T>) -> Option<T> {
    let first = iter.next()?;
    match iter.next() {
        Some(_) => None,
        None => Some(first),
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

pub fn close_window(id: u64) {
    if let Err(e) = send_action(niri_ipc::Action::CloseWindow { id: Some(id) }) {
        eprintln!("niri: close window {id}: {e}");
    }
}

/// Move a window to workspace `target_ws` and focus it — a notification's "move
/// here" action.
pub fn move_window_here(id: u64, target_ws: u64) {
    let action = niri_ipc::Action::MoveWindowToWorkspace {
        window_id: Some(id),
        reference: niri_ipc::WorkspaceReferenceArg::Id(target_ws),
        focus: true,
    };
    if let Err(e) = send_action(action) {
        eprintln!("niri: move window {id} to workspace {target_ws}: {e}");
    }
}

/// Power off all monitors (DPMS off), niri's own `power-off-monitors` action —
/// the idle `screen-off` subsystem. niri powers them back on at the next input,
/// so there is no matching power-on call here.
pub fn power_off_monitors() {
    if let Err(e) = send_action(niri_ipc::Action::PowerOffMonitors {}) {
        eprintln!("niri: power off monitors: {e}");
    }
}

/// Spawn a command through niri, so the resulting window lands in niri's scope
/// (odin's `niri msg action spawn`). The last-resort quick-launch path when an
/// app has neither a cwd-aware launch command nor a resolvable desktop entry.
pub fn spawn(command: Vec<String>) {
    if let Err(e) = send_action(niri_ipc::Action::Spawn {
        command: command.clone(),
    }) {
        eprintln!("niri: spawn {command:?}: {e}");
    }
}

/// The logical pixel size of one output.
#[derive(Debug, Clone, Copy)]
pub struct OutputGeometry {
    /// Logical width, used to place hover tooltips against the right screen
    /// edge (see `wayland_layers::tooltip::panel_left`).
    pub width: i32,
    pub height: i32,
}

/// The logical size of each connected output, by name, from a one-shot
/// `Outputs` query. The niri event stream carries no output sizes, but placing
/// a surface against the screen (e.g. centring a notification stack) needs
/// them. Empty on any failure (the caller then falls back to edge-anchored
/// placement).
pub fn output_geometry() -> HashMap<String, OutputGeometry> {
    let Ok(reply) = Socket::connect().and_then(|mut socket| socket.send(Request::Outputs)) else {
        return HashMap::new();
    };
    match reply {
        Ok(Response::Outputs(outputs)) => outputs
            .into_iter()
            .filter_map(|(name, output)| {
                output.logical.map(|logical| {
                    (
                        name,
                        OutputGeometry {
                            width: logical.width as i32,
                            height: logical.height as i32,
                        },
                    )
                })
            })
            .collect(),
        _ => HashMap::new(),
    }
}

fn send_action(action: niri_ipc::Action) -> Result<(), ActionError> {
    let reply = Socket::connect()?.send(Request::Action(action))?;
    reply.map_err(ActionError::Rejected)?;
    Ok(())
}
