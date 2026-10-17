use std::{collections::HashMap, convert::Infallible, fmt, future::ready, process, thread};

use iced::{
    Rectangle, Size,
    futures::{Stream, StreamExt},
};
use niri_ipc::{
    Event, Request, Response, Window,
    socket::Socket,
    state::{EventStreamState, EventStreamStatePart},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::data::source::{
    Delivery, LiveSource, Source, delivered,
    kitty_windows::WindowProcesses,
    window_label::{IconCandidate, WindowLabel},
};

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
pub struct WorkspaceIndicator {
    pub id: u64,
    pub idx: u8,
    pub name: Option<String>,
    pub is_active: bool,
    pub is_urgent: bool,
}

pub struct ActiveWorkspace<'a> {
    pub output: &'a str,
    pub name: Option<&'a str>,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    workspaces: HashMap<String, Vec<WorkspaceIndicator>>,
    windows: HashMap<u64, Vec<WinEntry>>,
    active: HashMap<String, u64>,
}

impl Source for Snapshot {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "niri";
}

impl LiveSource for Snapshot {
    fn events(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        delivered(ReceiverStream::new(spawn_event_thread()).scan(
            EventStreamState::default(),
            |state, event| {
                let _ = state.apply(event);
                ready(Some(Ok(Snapshot::derive(
                    state,
                    &WindowProcesses::default(),
                ))))
            },
        ))
    }
}

impl Snapshot {
    pub fn derive(state: &EventStreamState, processes: &WindowProcesses) -> Self {
        let mut workspaces: HashMap<String, Vec<WorkspaceIndicator>> = HashMap::new();
        let mut active = HashMap::new();
        for w in state.workspaces.workspaces.values() {
            let Some(output) = &w.output else { continue };
            if w.is_active {
                active.insert(output.clone(), w.id);
            }
            workspaces
                .entry(output.clone())
                .or_default()
                .push(WorkspaceIndicator {
                    id: w.id,
                    idx: w.idx,
                    name: w.name.clone(),
                    is_active: w.is_active,
                    is_urgent: w.is_urgent,
                });
        }
        for indicators in workspaces.values_mut() {
            indicators.sort_by_key(|indicator| indicator.idx);
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
                let entries = group.into_iter().map(|w| win_entry(w, processes)).collect();
                (workspace, entries)
            })
            .collect();

        Self {
            workspaces,
            windows,
            active,
        }
    }

    pub fn workspaces_for_output(&self, output: &str) -> &[WorkspaceIndicator] {
        self.workspaces.get(output).map_or(&[], Vec::as_slice)
    }

    pub fn windows_on_workspace(&self, workspace: u64) -> &[WinEntry] {
        self.windows.get(&workspace).map_or(&[], Vec::as_slice)
    }

    pub fn all_windows(&self) -> impl Iterator<Item = &WinEntry> {
        self.windows.values().flatten()
    }

    pub fn active_workspace_id(&self, output: &str) -> Option<u64> {
        self.active.get(output).copied()
    }

    pub fn active_workspaces(&self) -> impl Iterator<Item = ActiveWorkspace<'_>> {
        self.workspaces.iter().filter_map(|(output, indicators)| {
            let indicator = indicators.iter().find(|indicator| indicator.is_active)?;
            Some(ActiveWorkspace {
                output,
                name: indicator.name.as_deref(),
            })
        })
    }

    pub fn open_app_ids(&self, output: &str) -> Vec<String> {
        let Some(id) = self.active_workspace_id(output) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = self
            .windows_on_workspace(id)
            .iter()
            .filter_map(|win| win.app_id.clone())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    #[must_use]
    pub fn with_workspace_moved(mut self, workspace: u64, to_idx: usize) -> Self {
        let Some(indicators) = self
            .workspaces
            .values_mut()
            .find(|indicators| indicators.iter().any(|indicator| indicator.id == workspace))
        else {
            return self;
        };
        let Some(from) = indicators
            .iter()
            .position(|indicator| indicator.id == workspace)
        else {
            return self;
        };
        let moved = indicators.remove(from);
        indicators.insert(to_idx.saturating_sub(1).min(indicators.len()), moved);
        indicators
            .iter_mut()
            .zip(1..)
            .for_each(|(indicator, idx)| indicator.idx = idx);
        self
    }

    #[must_use]
    pub fn with_column_moved(mut self, window: u64, to_column: usize) -> Self {
        let Some(from_column) = self
            .windows
            .values()
            .flatten()
            .find(|entry| entry.id == window)
            .and_then(|entry| entry.layout.column)
        else {
            return self;
        };
        self.windows.values_mut().for_each(|entries| {
            let holds_window = entries.iter().any(|entry| entry.id == window);
            entries.iter_mut().for_each(|entry| {
                entry.is_focused = entry.id == window;
                if holds_window {
                    entry.layout.column = entry
                        .layout
                        .column
                        .map(|column| column_after_move(column, from_column, to_column));
                }
            });
            if holds_window {
                entries.sort_by_key(column_order);
            }
        });
        self
    }

    #[must_use]
    pub fn with_window_moved_to_workspace(mut self, window: u64, workspace: u64) -> Self {
        let Some(moved) = self.windows.values_mut().find_map(|entries| {
            let index = entries.iter().position(|entry| entry.id == window)?;
            Some(entries.remove(index))
        }) else {
            return self;
        };
        let target = self.windows.entry(workspace).or_default();
        let next_column = target
            .iter()
            .filter_map(|entry| entry.layout.column)
            .max()
            .map_or(1, |column| column + 1);
        target.push(WinEntry {
            is_focused: false,
            layout: WinLayout {
                column: moved.layout.column.map(|_| next_column),
                ..moved.layout.clone()
            },
            ..moved
        });
        target.sort_by_key(column_order);
        self
    }
}

fn column_order(entry: &WinEntry) -> usize {
    entry.layout.column.unwrap_or(usize::MAX)
}

fn column_after_move(column: usize, from: usize, to: usize) -> usize {
    if column == from {
        to
    } else if from < column && column <= to {
        column - 1
    } else if to <= column && column < from {
        column + 1
    } else {
        column
    }
}

fn exit_without_niri(reason: &dyn fmt::Display) -> ! {
    tracing::error!("niri: {reason}; the bar cannot run without niri events");
    process::exit(1)
}

pub fn spawn_event_thread() -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel(256);
    thread::spawn(move || {
        let mut socket = match Socket::connect() {
            Ok(socket) => socket,
            Err(e) => exit_without_niri(&format_args!("connect failed: {e}")),
        };
        if let Err(e) = socket.send(Request::EventStream) {
            exit_without_niri(&format_args!("EventStream request failed: {e}"));
        }
        let mut read = socket.read_events();
        loop {
            match read() {
                Ok(event) => {
                    if tx.blocking_send(event).is_err() {
                        return;
                    }
                }
                Err(e) => exit_without_niri(&format_args!("event read ended: {e}")),
            }
        }
    });
    rx
}

#[derive(Debug, Clone, Default)]
pub struct WinLayout {
    pub column: Option<usize>,
    pub tile_size: Size,
    pub is_floating: bool,
}

#[derive(Debug, Clone)]
pub struct WinEntry {
    pub id: u64,
    pub app_id: Option<String>,
    pub pid: Option<i32>,
    pub is_focused: bool,
    pub is_urgent: bool,
    pub layout: WinLayout,
    pub label: Option<WindowLabel>,
}

impl WinEntry {
    pub fn icon_candidates(&self) -> Vec<IconCandidate> {
        self.label
            .as_ref()
            .and_then(WindowLabel::theme_icon)
            .map(IconCandidate::Theme)
            .into_iter()
            .chain(self.app_id.clone().map(IconCandidate::AppId))
            .collect()
    }

    pub fn favicon_key(&self) -> Option<&str> {
        self.label.as_ref()?.favicon_key()
    }
}

fn win_entry(w: &niri_ipc::Window, processes: &WindowProcesses) -> WinEntry {
    let title = w.title.clone().filter(|title| !title.is_empty());
    let app_id = w.app_id.clone().filter(|app_id| !app_id.is_empty());
    let label = title.as_deref().or(app_id.as_deref()).map(|shown_title| {
        WindowLabel::classify(
            app_id.as_deref(),
            shown_title,
            processes.kitty_label_in(w.id).cloned(),
        )
    });
    WinEntry {
        id: w.id,
        app_id,
        pid: w.pid,
        is_focused: w.is_focused,
        is_urgent: w.is_urgent,
        label,
        layout: WinLayout {
            column: w.layout.pos_in_scrolling_layout.map(|(column, _)| column),
            tile_size: Size::new(w.layout.tile_size.0 as f32, w.layout.tile_size.1 as f32),
            is_floating: w.is_floating,
        },
    }
}

#[derive(Default)]
pub struct Placement {
    workspace_by_window: HashMap<u64, u64>,
    focused_window: Option<u64>,
    focused_output: Option<String>,
    output_by_workspace: HashMap<u64, String>,
    edges: HashMap<u64, WindowEdge>,
    rects: HashMap<u64, Rectangle>,
    windows: Vec<WindowIdentity>,
}

struct WindowIdentity {
    id: u64,
    app_id: Option<String>,
    pid: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowOwner {
    App(String),
    Process(u32),
}

impl WindowIdentity {
    fn owned_by(&self, owner: &WindowOwner) -> bool {
        match owner {
            WindowOwner::App(app_id) => self.app_id.as_ref() == Some(app_id),
            WindowOwner::Process(pid) => self
                .pid
                .is_some_and(|window_pid| u32::try_from(window_pid) == Ok(*pid)),
        }
    }
}

impl Placement {
    pub fn derive(state: &EventStreamState) -> Self {
        let mut ws_idx = HashMap::new();
        let mut output_by_workspace = HashMap::new();
        let mut active_ws_by_output: HashMap<String, OutputActiveWorkspace> = HashMap::new();
        let mut ws_active_window = HashMap::new();
        let mut focused_output = None;
        for w in state.workspaces.workspaces.values() {
            ws_idx.insert(w.id, w.idx);
            if let Some(output) = &w.output {
                output_by_workspace.insert(w.id, output.clone());
                if w.is_active {
                    active_ws_by_output.insert(
                        output.clone(),
                        OutputActiveWorkspace {
                            id: w.id,
                            idx: w.idx,
                        },
                    );
                }
                if w.is_focused {
                    focused_output = Some(output.clone());
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
                pid: w.pid,
            });
        }

        let edges = workspace_by_window
            .iter()
            .map(|(&id, &ws)| {
                let output = output_by_workspace.get(&ws).cloned();
                let active_ws = output.as_ref().and_then(|o| active_ws_by_output.get(o));
                let vertical =
                    vertical_edge(ws_idx.get(&ws).copied(), active_ws.map(|active| active.idx));
                let horizontal = if active_ws.is_some_and(|active| active.id == ws) {
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
            focused_output,
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

    pub fn focused_output(&self) -> Option<&str> {
        self.focused_output.as_deref()
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

struct OutputActiveWorkspace {
    id: u64,
    idx: u8,
}

fn viewed_rect(
    window: &Window,
    output_by_workspace: &HashMap<u64, String>,
    active_ws_by_output: &HashMap<String, OutputActiveWorkspace>,
) -> Option<Rectangle> {
    let workspace = window.workspace_id?;
    let output = output_by_workspace.get(&workspace)?;
    if active_ws_by_output
        .get(output)
        .is_none_or(|active| active.id != workspace)
    {
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
    pub fn window_owned_by(&self, owner: &WindowOwner) -> Option<u64> {
        let mut matching = self.windows.iter().filter(|window| window.owned_by(owner));
        unique(&mut matching).map(|window| window.id)
    }
}

fn unique<T>(iter: &mut impl Iterator<Item = T>) -> Option<T> {
    let first = iter.next()?;
    match iter.next() {
        Some(_) => None,
        None => Some(first),
    }
}

#[derive(Debug, Clone)]
pub enum ActionFailure {
    Rejected { action: String, reason: String },
    Unreachable { action: String, reason: String },
}

impl ActionFailure {
    pub fn summary(&self) -> &'static str {
        match self {
            ActionFailure::Rejected { .. } => "niri refused a request from the bar",
            ActionFailure::Unreachable { .. } => "The bar could not reach niri",
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            ActionFailure::Rejected { reason, .. } | ActionFailure::Unreachable { reason, .. } => {
                reason
            }
        }
    }

    pub fn action(&self) -> &str {
        match self {
            ActionFailure::Rejected { action, .. } | ActionFailure::Unreachable { action, .. } => {
                action
            }
        }
    }
}

impl fmt::Display for ActionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({})",
            self.summary(),
            self.reason(),
            self.action()
        )
    }
}

pub fn send_action(action: &niri_ipc::Action) -> Result<(), ActionFailure> {
    let sent =
        Socket::connect().and_then(|mut socket| socket.send(Request::Action(action.clone())));
    match sent {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(rejection)) => Err(ActionFailure::Rejected {
            action: format!("{action:?}"),
            reason: rejection,
        }),
        Err(e) => Err(ActionFailure::Unreachable {
            action: format!("{action:?}"),
            reason: e.to_string(),
        }),
    }
}

pub fn send_action_or_warn(action: &niri_ipc::Action) {
    if let Err(failure) = send_action(action) {
        tracing::warn!("{failure}");
    }
}

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub mode: Option<OutputMode>,
    pub scale: f64,
    pub logical_height: u32,
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

fn product_field(edid_field: String) -> Option<String> {
    describes_a_product(&edid_field).then_some(edid_field)
}

fn current_mode(output: &niri_ipc::Output) -> Option<OutputMode> {
    let mode = output.modes.get(output.current_mode?)?;
    Some(OutputMode {
        width: mode.width,
        height: mode.height,
        refresh_hz: f64::from(mode.refresh_rate) / 1000.0,
    })
}

pub fn version() -> Option<String> {
    let reply = Socket::connect()
        .and_then(|mut socket| socket.send(Request::Version))
        .ok()?;
    match reply {
        Ok(Response::Version(version)) => Some(version),
        _ => None,
    }
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
                        serial: output.serial.and_then(product_field),
                        mode,
                        scale: logical.scale,
                        logical_height: logical.height,
                    },
                ))
            })
            .collect(),
        _ => HashMap::new(),
    }
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
