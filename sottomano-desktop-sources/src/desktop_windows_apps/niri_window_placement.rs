use std::collections::HashMap;

use niri_ipc::{Window, WindowLayout, state::EventStreamState};
use sottomano_toml_config::screen_edges::{HEdge, VEdge};

#[derive(Debug, Clone)]
pub struct WindowEdge {
    pub vertical: VEdge,
    pub horizontal: HEdge,
    pub output: Option<String>,
}

#[derive(Default)]
pub struct Placement {
    workspace_by_window: HashMap<u64, u64>,
    focused_window: Option<u64>,
    focused_output: Option<String>,
    output_by_workspace: HashMap<u64, String>,
    edges: HashMap<u64, WindowEdge>,
    viewed_layouts: HashMap<u64, WindowLayout>,
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
    #[must_use]
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
        let mut viewed_layouts = HashMap::new();
        for w in state.windows.windows.values() {
            if let Some(ws) = w.workspace_id {
                workspace_by_window.insert(w.id, ws);
            }
            if !state.overview.is_open
                && on_viewed_workspace(w, &output_by_workspace, &active_ws_by_output)
            {
                viewed_layouts.insert(w.id, w.layout.clone());
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
            viewed_layouts,
            windows,
        }
    }

    #[must_use]
    pub fn viewed_window_layout(&self, id: u64) -> Option<&WindowLayout> {
        self.viewed_layouts.get(&id)
    }

    #[must_use]
    pub fn window_edge(&self, id: u64) -> WindowEdge {
        self.edges.get(&id).cloned().unwrap_or(WindowEdge {
            vertical: VEdge::Center,
            horizontal: HEdge::Center,
            output: None,
        })
    }

    #[must_use]
    pub fn focused_window(&self) -> Option<u64> {
        self.focused_window
    }

    #[must_use]
    pub fn focused_output(&self) -> Option<&str> {
        self.focused_output.as_deref()
    }

    #[must_use]
    pub fn workspace_of_window(&self, id: u64) -> Option<u64> {
        self.workspace_by_window.get(&id).copied()
    }

    pub fn output_of_workspace(&self, ws: u64) -> Option<&str> {
        self.output_by_workspace.get(&ws).map(String::as_str)
    }

    #[must_use]
    pub fn active_workspace_for_move(&self) -> Option<u64> {
        self.workspace_by_window.get(&self.focused_window?).copied()
    }

    #[must_use]
    pub fn window_owned_by(&self, owner: &WindowOwner) -> Option<u64> {
        let mut matching = self.windows.iter().filter(|window| window.owned_by(owner));
        unique(&mut matching).map(|window| window.id)
    }
}

struct OutputActiveWorkspace {
    id: u64,
    idx: u8,
}

fn on_viewed_workspace(
    window: &Window,
    output_by_workspace: &HashMap<u64, String>,
    active_ws_by_output: &HashMap<String, OutputActiveWorkspace>,
) -> bool {
    window.workspace_id.is_some_and(|workspace| {
        output_by_workspace
            .get(&workspace)
            .and_then(|output| active_ws_by_output.get(output))
            .is_some_and(|active| active.id == workspace)
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

fn unique<T>(iter: &mut impl Iterator<Item = T>) -> Option<T> {
    let first = iter.next()?;
    match iter.next() {
        Some(_) => None,
        None => Some(first),
    }
}
