use std::{cmp::Ordering, collections::HashMap};

use sottomano_compositor_model::compositor_window_state::{
    CompositorWindow, CompositorWindowLayout, CompositorWindowState,
};

use crate::desktop_windows_apps::{
    terminal_emulator_windows::WindowProcesses,
    window_title_labels::{IconCandidate, WindowGroupKey, WindowLabel},
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

impl Snapshot {
    #[must_use]
    pub fn derive(state: &CompositorWindowState, processes: &WindowProcesses) -> Self {
        let mut workspaces: HashMap<String, Vec<WorkspaceIndicator>> = HashMap::new();
        let mut active = HashMap::new();
        for w in state.workspaces.values() {
            let Some(output) = &w.output else { continue };
            if w.is_active {
                active.insert(output.clone(), w.id);
            }
            workspaces
                .entry(output.clone())
                .or_default()
                .push(WorkspaceIndicator {
                    id: w.id,
                    idx: w.index,
                    name: w.name.clone(),
                    is_active: w.is_active,
                    is_urgent: w.is_urgent,
                });
        }
        for indicators in workspaces.values_mut() {
            indicators.sort_by_key(|indicator| indicator.idx);
        }

        let mut windows: HashMap<u64, Vec<WinEntry>> = HashMap::new();
        for compositor_window in state.windows.values() {
            let Some(workspace) = compositor_window.workspace_id else {
                continue;
            };
            windows
                .entry(workspace)
                .or_default()
                .push(win_entry(compositor_window, processes));
        }
        for entries in windows.values_mut() {
            entries.sort_by(compare_reading_order);
        }

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

    pub fn active_workspace_ids_by_output(&self) -> impl Iterator<Item = (&str, u64)> {
        self.active
            .iter()
            .map(|(output, workspace_id)| (output.as_str(), *workspace_id))
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
        let to_position = to_idx
            .checked_sub(1)
            .expect("compositor workspace indices start at 1");
        indicators.insert(to_position.min(indicators.len()), moved);
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
            .and_then(|entry| entry.layout.column())
        else {
            return self;
        };
        self.windows.values_mut().for_each(|entries| {
            let holds_window = entries.iter().any(|entry| entry.id == window);
            entries.iter_mut().for_each(|entry| {
                entry.is_focused = entry.id == window;
                if holds_window
                    && let WorkspacePlacement::ScrollingTile { column, .. } =
                        &mut entry.layout.placement
                {
                    *column = column_after_move(*column, from_column, to_column);
                }
            });
            if holds_window {
                entries.sort_by(compare_reading_order);
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
            .filter_map(|entry| entry.layout.column())
            .max()
            .map_or(1, |column| column + 1);
        let placement = match moved.layout.placement {
            WorkspacePlacement::ScrollingTile { .. } => WorkspacePlacement::ScrollingTile {
                column: next_column,
                tile: 1,
            },
            WorkspacePlacement::ScreenPosition { .. } | WorkspacePlacement::Unplaced => {
                WorkspacePlacement::Unplaced
            }
        };
        target.push(WinEntry {
            is_focused: false,
            layout: WinLayout {
                placement,
                ..moved.layout.clone()
            },
            ..moved
        });
        target.sort_by(compare_reading_order);
        self
    }
}

fn compare_reading_order(left: &WinEntry, right: &WinEntry) -> Ordering {
    left.layout
        .is_floating
        .cmp(&right.layout.is_floating)
        .then_with(|| {
            left.layout
                .placement
                .compare_reading_order(&right.layout.placement)
        })
        .then_with(|| left.id.cmp(&right.id))
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum WorkspacePlacement {
    ScrollingTile {
        column: usize,
        tile: usize,
    },
    ScreenPosition {
        horizontal: f64,
        vertical: f64,
    },
    #[default]
    Unplaced,
}

impl WorkspacePlacement {
    fn from_compositor_layout(layout: &CompositorWindowLayout) -> Self {
        match (layout.scrolling_position, layout.tile_position_in_view) {
            (Some((column, tile)), _) => WorkspacePlacement::ScrollingTile { column, tile },
            (None, Some((horizontal, vertical))) => WorkspacePlacement::ScreenPosition {
                horizontal,
                vertical,
            },
            (None, None) => WorkspacePlacement::Unplaced,
        }
    }

    fn reading_rank(self) -> u8 {
        match self {
            WorkspacePlacement::ScrollingTile { .. } => 0,
            WorkspacePlacement::ScreenPosition { .. } => 1,
            WorkspacePlacement::Unplaced => 2,
        }
    }

    fn compare_reading_order(&self, other: &Self) -> Ordering {
        match (self, other) {
            (
                WorkspacePlacement::ScrollingTile {
                    column: left_column,
                    tile: left_tile,
                },
                WorkspacePlacement::ScrollingTile {
                    column: right_column,
                    tile: right_tile,
                },
            ) => (left_column, left_tile).cmp(&(right_column, right_tile)),
            (
                WorkspacePlacement::ScreenPosition {
                    horizontal: left_horizontal,
                    vertical: left_vertical,
                },
                WorkspacePlacement::ScreenPosition {
                    horizontal: right_horizontal,
                    vertical: right_vertical,
                },
            ) => left_horizontal
                .total_cmp(right_horizontal)
                .then_with(|| left_vertical.total_cmp(right_vertical)),
            _ => self.reading_rank().cmp(&other.reading_rank()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WinLayout {
    pub placement: WorkspacePlacement,
    pub tile_size: (f64, f64),
    pub is_floating: bool,
}

impl WinLayout {
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        match self.placement {
            WorkspacePlacement::ScrollingTile { column, .. } => Some(column),
            WorkspacePlacement::ScreenPosition { .. } | WorkspacePlacement::Unplaced => None,
        }
    }
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

    #[must_use]
    pub fn program_name(&self) -> Option<&str> {
        self.label.as_ref()?.program_name()
    }

    #[must_use]
    pub fn group_key(&self) -> Option<WindowGroupKey<'_>> {
        self.label
            .as_ref()
            .and_then(WindowLabel::group_key)
            .or_else(|| self.app_id.as_deref().map(WindowGroupKey::Application))
    }
}

fn win_entry(w: &CompositorWindow, processes: &WindowProcesses) -> WinEntry {
    let title = w.title.clone().filter(|title| !title.is_empty());
    let app_id = w.app_id.clone().filter(|app_id| !app_id.is_empty());
    let label = match processes.terminal_label_in(w.id) {
        Some(terminal_label) => Some(WindowLabel::TerminalShell(terminal_label.clone())),
        None => match app_id.as_deref() {
            Some(app_id) => Some(WindowLabel::classify(
                app_id,
                title.as_deref().unwrap_or(app_id),
            )),
            None => title.map(WindowLabel::Titled),
        },
    };
    WinEntry {
        id: w.id,
        app_id,
        pid: w.pid,
        is_focused: w.is_focused,
        is_urgent: w.is_urgent,
        label,
        layout: WinLayout {
            placement: WorkspacePlacement::from_compositor_layout(&w.layout),
            tile_size: w.layout.tile_size,
            is_floating: w.is_floating,
        },
    }
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::compositor_window_state::{
        CompositorWindow, CompositorWindowLayout, CompositorWindowState,
    };

    use super::Snapshot;
    use crate::desktop_windows_apps::terminal_emulator_windows::WindowProcesses;

    const SHARED_WORKSPACE: u64 = 1;

    fn placed_window(
        id: u64,
        scrolling_position: Option<(usize, usize)>,
        tile_position_in_view: Option<(f64, f64)>,
        is_floating: bool,
    ) -> CompositorWindow {
        CompositorWindow {
            id,
            title: None,
            app_id: Some("kitty".to_owned()),
            pid: None,
            workspace_id: Some(SHARED_WORKSPACE),
            is_focused: false,
            is_floating,
            is_urgent: false,
            layout: CompositorWindowLayout {
                scrolling_position,
                tile_position_in_view,
                tile_size: (800.0, 600.0),
                ..CompositorWindowLayout::default()
            },
            focused_at_monotonic: None,
        }
    }

    fn ordered_window_ids(windows: Vec<CompositorWindow>) -> Vec<u64> {
        let state = CompositorWindowState::indexed_by_id([], windows);
        Snapshot::derive(&state, &WindowProcesses::default())
            .windows_on_workspace(SHARED_WORKSPACE)
            .iter()
            .map(|window_entry| window_entry.id)
            .collect()
    }

    #[test]
    fn hyprland_windows_read_left_to_right_then_top_to_bottom_with_floating_last() {
        let ordered = ordered_window_ids(vec![
            placed_window(1, None, Some((100.0, 50.0)), true),
            placed_window(2, None, Some((960.0, 540.0)), false),
            placed_window(3, None, Some((960.0, 10.0)), false),
            placed_window(4, None, Some((10.0, 10.0)), false),
        ]);
        assert_eq!(ordered, [4, 3, 2, 1]);
    }

    #[test]
    fn niri_windows_keep_their_column_and_tile_order() {
        let ordered = ordered_window_ids(vec![
            placed_window(1, Some((2, 1)), Some((0.0, 0.0)), false),
            placed_window(2, Some((1, 2)), Some((900.0, 0.0)), false),
            placed_window(3, Some((1, 1)), Some((900.0, 900.0)), false),
            placed_window(4, None, Some((0.0, 0.0)), true),
        ]);
        assert_eq!(ordered, [3, 2, 1, 4]);
    }
}
