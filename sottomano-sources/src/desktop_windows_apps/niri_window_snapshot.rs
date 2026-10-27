use std::{collections::HashMap, convert::Infallible, fmt, future::ready, process, thread};

use futures::{Stream, StreamExt};
use niri_ipc::{
    Event, Request,
    socket::Socket,
    state::{EventStreamState, EventStreamStatePart},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    desktop_windows_apps::{
        kitty_terminal_windows::WindowProcesses,
        window_title_labels::{IconCandidate, WindowLabel},
    },
};

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

impl SnapshotSource for Snapshot {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Niri windows";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        ReceiverStream::new(spawn_event_thread())
            .scan(EventStreamState::default(), |state, event| {
                let _ = state.apply(event);
                ready(Some(Ok(Snapshot::derive(
                    state,
                    &WindowProcesses::default(),
                ))))
            })
            .map(SnapshotDelivery::live_push)
    }
}

impl Snapshot {
    #[must_use]
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

    #[must_use]
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

    #[must_use]
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

#[must_use]
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
    pub tile_size: (f64, f64),
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

    #[must_use]
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
            tile_size: w.layout.tile_size,
            is_floating: w.is_floating,
        },
    }
}
