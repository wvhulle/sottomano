use std::{collections::HashMap, fmt, io::Error, thread};

use iced::Rectangle;
use niri_ipc::{Event, Request, Response, Window, socket::Socket, state::EventStreamState};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VEdge {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HEdge {
    Left,
    Center,
    Right,
}

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

    pub fn workspaces_for_output(&self, output: &str) -> &[WsPill] {
        self.workspaces.get(output).map_or(&[], Vec::as_slice)
    }

    pub fn workspace(&self, id: u64) -> Option<&WsPill> {
        self.workspaces
            .values()
            .flatten()
            .find(|pill| pill.id == id)
    }

    pub fn windows_on_workspace(&self, workspace: u64) -> &[WinEntry] {
        self.windows.get(&workspace).map_or(&[], Vec::as_slice)
    }

    pub fn active_workspace_id(&self, output: &str) -> Option<u64> {
        self.active.get(output).copied()
    }

    pub fn active_workspace_name(&self, output: &str) -> Option<&str> {
        let id = self.active_workspace_id(output)?;
        self.workspaces
            .get(output)?
            .iter()
            .find(|pill| pill.id == id)
            .and_then(|pill| pill.name.as_deref())
    }

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
                        return;
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
    pub pid: Option<i32>,
    pub is_focused: bool,
    pub is_urgent: bool,
}

pub fn all_windows(state: &EventStreamState) -> Vec<WinEntry> {
    state.windows.windows.values().map(win_entry).collect()
}

fn win_entry(w: &niri_ipc::Window) -> WinEntry {
    WinEntry {
        id: w.id,
        title: w.title.clone().unwrap_or_default(),
        app_id: w.app_id.clone().unwrap_or_default(),
        pid: w.pid,
        is_focused: w.is_focused,
        is_urgent: w.is_urgent,
    }
}

#[derive(Default)]
pub struct Placement {
    workspace_by_window: HashMap<u64, u64>,
    focused_window: Option<u64>,
    output_by_workspace: HashMap<u64, String>,
    edges: HashMap<u64, WindowEdge>,
    rects: HashMap<u64, Rectangle>,
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
        let mut rects = HashMap::new();
        for w in state.windows.windows.values() {
            if let Some(ws) = w.workspace_id {
                workspace_by_window.insert(w.id, ws);
            }
            if !state.overview.is_open
                && let Some(rect) = viewed_rect(w, &output_by_workspace, &active_ws_by_output)
            {
                rects.insert(w.id, rect);
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
            rects,
            windows,
        }
    }

    pub fn window_rect(&self, id: u64) -> Option<Rectangle> {
        self.rects.get(&id).copied()
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

    pub fn workspace_of_window(&self, id: u64) -> Option<u64> {
        self.workspace_by_window.get(&id).copied()
    }

    pub fn output_of_workspace(&self, ws: u64) -> Option<&str> {
        self.output_by_workspace.get(&ws).map(String::as_str)
    }

    pub fn active_workspace_for_move(&self) -> Option<u64> {
        self.workspace_by_window.get(&self.focused_window?).copied()
    }
}

fn viewed_rect(
    window: &Window,
    output_by_workspace: &HashMap<u64, String>,
    active_ws_by_output: &HashMap<String, u64>,
) -> Option<Rectangle> {
    let workspace = window.workspace_id?;
    let output = output_by_workspace.get(&workspace)?;
    if active_ws_by_output.get(output) != Some(&workspace) {
        return None;
    }
    let (tile_x, tile_y) = window.layout.tile_pos_in_workspace_view?;
    let (offset_x, offset_y) = window.layout.window_offset_in_tile;
    let (width, height) = window.layout.window_size;
    Some(Rectangle {
        x: (tile_x + offset_x) as f32,
        y: (tile_y + offset_y) as f32,
        width: width as f32,
        height: height as f32,
    })
}

fn vertical_edge(window_ws_idx: Option<u8>, active_ws_idx: Option<u8>) -> VEdge {
    match (window_ws_idx, active_ws_idx) {
        (Some(w), Some(a)) if w < a => VEdge::Top,
        (Some(w), Some(a)) if w > a => VEdge::Bottom,
        _ => VEdge::Center,
    }
}

fn horizontal_edge(window_col: Option<usize>, ws_focused_col: Option<usize>) -> HEdge {
    match (window_col, ws_focused_col) {
        (Some(w), Some(f)) if w < f => HEdge::Left,
        (Some(w), Some(f)) if w > f => HEdge::Right,
        _ => HEdge::Center,
    }
}

impl Placement {
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

pub fn toggle_overview() {
    if let Err(e) = send_action(niri_ipc::Action::ToggleOverview {}) {
        eprintln!("niri: toggle overview: {e}");
    }
}

pub fn close_window(id: u64) {
    if let Err(e) = send_action(niri_ipc::Action::CloseWindow { id: Some(id) }) {
        eprintln!("niri: close window {id}: {e}");
    }
}

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

pub fn power_off_monitors() {
    if let Err(e) = send_action(niri_ipc::Action::PowerOffMonitors {}) {
        eprintln!("niri: power off monitors: {e}");
    }
}

pub fn spawn(command: &[String]) {
    if let Err(e) = send_action(niri_ipc::Action::Spawn {
        command: command.to_vec(),
    }) {
        eprintln!("niri: spawn {command:?}: {e}");
    }
}

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub make: String,
    pub model: String,
    pub mode: Option<OutputMode>,
    pub scale: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputMode {
    pub width: u16,
    pub height: u16,
    pub refresh_hz: f64,
}

fn describes_a_product(edid_field: &str) -> bool {
    let trimmed = edid_field.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        return false;
    }
    !trimmed
        .strip_prefix("0x")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

fn product_field(edid_field: String) -> String {
    if describes_a_product(&edid_field) {
        edid_field
    } else {
        String::new()
    }
}

fn current_mode(output: &niri_ipc::Output) -> Option<OutputMode> {
    let mode = output.modes.get(output.current_mode?)?;
    Some(OutputMode {
        width: mode.width,
        height: mode.height,
        refresh_hz: f64::from(mode.refresh_rate) / 1000.0,
    })
}

pub fn output_info() -> HashMap<String, OutputInfo> {
    let Ok(reply) = Socket::connect().and_then(|mut socket| socket.send(Request::Outputs)) else {
        return HashMap::new();
    };
    match reply {
        Ok(Response::Outputs(outputs)) => outputs
            .into_iter()
            .filter_map(|(name, output)| {
                let logical = output.logical?;
                let mode = current_mode(&output);
                Some((
                    name,
                    OutputInfo {
                        make: product_field(output.make),
                        model: product_field(output.model),
                        mode,
                        scale: logical.scale,
                    },
                ))
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

#[cfg(test)]
mod tests {
    use super::describes_a_product;

    #[test]
    fn edid_product_codes_are_not_product_names() {
        assert!(!describes_a_product("0x07BE"));
        assert!(!describes_a_product("0x0000ffa1"));
        assert!(!describes_a_product(""));
        assert!(!describes_a_product("   "));
        assert!(!describes_a_product("Unknown"));
    }

    #[test]
    fn real_edid_strings_are_product_names() {
        assert!(describes_a_product("LG Display"));
        assert!(describes_a_product("U2720Q"));
        assert!(describes_a_product("0xSomething"));
    }
}
