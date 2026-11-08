use std::{
    collections::{BTreeSet, HashMap},
    io,
};

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::{
    compositor_window_ipc::{
        compositor_output_facts::{OutputInfo, OutputMode, product_field},
        compositor_window_state::{
            CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowLayout,
            CompositorWindowState, CompositorWorkspace,
        },
        sway_ipc_connection::{SwayEvent, SwayEventKind, SwayIpcConnection, SwayMessageKind},
    },
    desktop_windows_apps::process_tree_inspection,
};

const SCRATCHPAD_OUTPUT: &str = "__i3";

#[derive(Debug, Clone, Copy, Default, Deserialize)]
struct SwayRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Deserialize)]
struct SwayWindowProperties {
    class: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SwayNode {
    id: u64,
    #[serde(rename = "type")]
    node_type: String,
    name: Option<String>,
    app_id: Option<String>,
    pid: Option<i32>,
    #[serde(default)]
    focused: bool,
    #[serde(default)]
    urgent: bool,
    #[serde(default)]
    nodes: Vec<SwayNode>,
    #[serde(default)]
    floating_nodes: Vec<SwayNode>,
    #[serde(default)]
    focus: Vec<u64>,
    #[serde(default)]
    rect: SwayRect,
    window_rect: Option<SwayRect>,
    window_properties: Option<SwayWindowProperties>,
}

#[derive(Debug, Deserialize)]
struct SwayWorkspace {
    id: u64,
    num: i32,
    name: String,
    visible: bool,
    focused: bool,
    urgent: bool,
    output: String,
}

#[derive(Debug, Deserialize)]
struct SwayChangeEvent {
    change: String,
}

#[derive(Debug, Deserialize)]
struct SwayOutputMode {
    width: u16,
    height: u16,
    refresh: i32,
}

#[derive(Debug, Deserialize)]
struct SwayOutput {
    name: String,
    #[serde(default)]
    make: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    serial: String,
    #[serde(default)]
    active: bool,
    current_mode: Option<SwayOutputMode>,
    scale: Option<f64>,
    #[serde(default)]
    rect: SwayRect,
}

#[derive(Debug, Deserialize)]
struct SwayVersion {
    human_readable: String,
}

impl SwayNode {
    fn node_role(&self) -> SwayNodeRole {
        if matches!(self.node_type.as_str(), "con" | "floating_con")
            && self.nodes.is_empty()
            && self.floating_nodes.is_empty()
            && (self.pid.is_some() || self.app_id.is_some() || self.window_properties.is_some())
        {
            SwayNodeRole::Window
        } else {
            SwayNodeRole::Container
        }
    }

    fn children(&self) -> impl Iterator<Item = &SwayNode> {
        self.nodes.iter().chain(&self.floating_nodes)
    }

    fn focused_window(&self) -> Option<u64> {
        if self.node_role() == SwayNodeRole::Window {
            return Some(self.id);
        }
        let focused_child_id = self.focus.first()?;
        self.children()
            .find(|child| child.id == *focused_child_id)?
            .focused_window()
    }

    fn app_id(&self) -> Option<String> {
        self.app_id
            .clone()
            .or_else(|| self.window_properties.as_ref()?.class.clone())
    }
}

struct PlacedSwayWorkspace<'tree> {
    node: &'tree SwayNode,
    output_rect: SwayRect,
}

fn placed_workspaces(tree: &SwayNode) -> HashMap<u64, PlacedSwayWorkspace<'_>> {
    tree.nodes
        .iter()
        .filter(|output| output.name.as_deref() != Some(SCRATCHPAD_OUTPUT))
        .flat_map(|output| {
            output
                .nodes
                .iter()
                .filter(|workspace| workspace.node_type == "workspace")
                .map(|workspace| {
                    (
                        workspace.id,
                        PlacedSwayWorkspace {
                            node: workspace,
                            output_rect: output.rect,
                        },
                    )
                })
        })
        .collect()
}

fn collect_windows(
    node: &SwayNode,
    workspace_id: u64,
    output_rect: SwayRect,
    is_floating: bool,
    windows: &mut Vec<CompositorWindow>,
) {
    if node.node_role() == SwayNodeRole::Window {
        windows.push(sway_window_of(node, workspace_id, output_rect, is_floating));
        return;
    }
    node.nodes
        .iter()
        .for_each(|child| collect_windows(child, workspace_id, output_rect, is_floating, windows));
    node.floating_nodes
        .iter()
        .for_each(|child| collect_windows(child, workspace_id, output_rect, true, windows));
}

fn sway_window_of(
    node: &SwayNode,
    workspace_id: u64,
    output_rect: SwayRect,
    is_floating: bool,
) -> CompositorWindow {
    let window_rect = node.window_rect.unwrap_or_default();
    CompositorWindow {
        id: node.id,
        title: node.name.clone(),
        app_id: node.app_id(),
        pid: node.pid,
        workspace_id: Some(workspace_id),
        is_focused: node.focused,
        is_floating,
        is_urgent: node.urgent,
        layout: CompositorWindowLayout {
            scrolling_position: None,
            tile_size: (f64::from(node.rect.width), f64::from(node.rect.height)),
            window_size: (window_rect.width, window_rect.height),
            tile_position_in_view: Some((
                f64::from(node.rect.x - output_rect.x),
                f64::from(node.rect.y - output_rect.y),
            )),
            window_offset_in_tile: (f64::from(window_rect.x), f64::from(window_rect.y)),
        },
        focused_at_monotonic: None,
    }
}

fn given_workspace_name(workspace: &SwayWorkspace) -> Option<String> {
    (workspace.name != workspace.num.to_string()).then(|| workspace.name.clone())
}

fn sway_state_of(workspaces: &[SwayWorkspace], tree: &SwayNode) -> CompositorWindowState {
    let placed = placed_workspaces(tree);
    let mut next_index_by_output: HashMap<&str, u8> = HashMap::new();
    let compositor_workspaces: Vec<CompositorWorkspace> = workspaces
        .iter()
        .filter(|workspace| placed.contains_key(&workspace.id))
        .map(|workspace| {
            let index = next_index_by_output
                .entry(workspace.output.as_str())
                .and_modify(|index| *index = index.saturating_add(1))
                .or_insert(1);
            CompositorWorkspace {
                id: workspace.id,
                index: *index,
                name: given_workspace_name(workspace),
                output: Some(workspace.output.clone()),
                is_urgent: workspace.urgent,
                is_active: workspace.visible,
                is_focused: workspace.focused,
                active_window_id: placed
                    .get(&workspace.id)
                    .and_then(|placed_workspace| placed_workspace.node.focused_window()),
            }
        })
        .collect();
    let mut windows = Vec::new();
    for (workspace_id, placed_workspace) in &placed {
        collect_windows(
            placed_workspace.node,
            *workspace_id,
            placed_workspace.output_rect,
            false,
            &mut windows,
        );
    }
    CompositorWindowState::from_lists(compositor_workspaces, windows)
}

fn read_sway_state(queries: &mut SwayIpcConnection) -> io::Result<CompositorWindowState> {
    let workspaces: Vec<SwayWorkspace> =
        serde_json::from_str(&queries.request(SwayMessageKind::GetWorkspaces, "")?)?;
    let tree: SwayNode = serde_json::from_str(&queries.request(SwayMessageKind::GetTree, "")?)?;
    Ok(sway_state_of(&workspaces, &tree))
}

fn sway_changes_of(event: &SwayEvent) -> BTreeSet<CompositorChange> {
    let change = serde_json::from_str::<SwayChangeEvent>(&event.payload)
        .map(|parsed| parsed.change)
        .unwrap_or_default();
    match (event.kind, change.as_str()) {
        (SwayEventKind::Window, "focus") => [CompositorChange::Focus].into(),
        (SwayEventKind::Window, _) => [CompositorChange::Windows].into(),
        (SwayEventKind::Workspace, "focus") => {
            [CompositorChange::Workspaces, CompositorChange::Focus].into()
        }
        (SwayEventKind::Workspace | SwayEventKind::Output, _) => {
            [CompositorChange::Workspaces].into()
        }
        (SwayEventKind::Shutdown | SwayEventKind::Other, _) => BTreeSet::new(),
    }
}

pub fn follow_sway_events(sender: &mpsc::Sender<CompositorUpdate>) -> io::Result<()> {
    let mut subscription =
        SwayIpcConnection::connect()?.subscribe(&["window", "workspace", "output", "shutdown"])?;
    let mut queries = SwayIpcConnection::connect()?;
    let mut previous = CompositorWindowState::default();
    let mut changes: BTreeSet<CompositorChange> = CompositorChange::ALL.into();
    loop {
        let state = read_sway_state(&mut queries)?
            .with_focus_times_from(&previous, process_tree_inspection::monotonic());
        previous = state.clone();
        if sender
            .blocking_send(CompositorUpdate { state, changes })
            .is_err()
        {
            return Ok(());
        }
        let event = subscription.next_event()?;
        if event.kind == SwayEventKind::Shutdown {
            return Err(io::Error::other("sway is shutting down"));
        }
        changes = sway_changes_of(&event);
    }
}

#[must_use]
pub fn sway_outputs() -> HashMap<String, OutputInfo> {
    let Ok(reply) = SwayIpcConnection::connect()
        .and_then(|mut connection| connection.request(SwayMessageKind::GetOutputs, ""))
    else {
        return HashMap::new();
    };
    let Ok(outputs) = serde_json::from_str::<Vec<SwayOutput>>(&reply) else {
        return HashMap::new();
    };
    outputs
        .into_iter()
        .filter(|output| output.active)
        .map(|output| {
            let facts = OutputInfo {
                make: product_field(output.make),
                model: product_field(output.model),
                serial: product_field(output.serial),
                mode: output.current_mode.map(|mode| OutputMode {
                    width: mode.width,
                    height: mode.height,
                    refresh_hz: f64::from(mode.refresh) / 1000.0,
                }),
                scale: output.scale.unwrap_or(1.0),
                logical_width: u32::try_from(output.rect.width).unwrap_or(0),
                logical_height: u32::try_from(output.rect.height).unwrap_or(0),
            };
            (output.name, facts)
        })
        .collect()
}

#[must_use]
pub fn sway_version() -> Option<String> {
    let reply = SwayIpcConnection::connect()
        .and_then(|mut connection| connection.request(SwayMessageKind::GetVersion, ""))
        .ok()?;
    Some(
        serde_json::from_str::<SwayVersion>(&reply)
            .ok()?
            .human_readable,
    )
}

#[must_use]
pub fn sway_window_pids() -> Option<Vec<i32>> {
    let state = read_sway_state(&mut SwayIpcConnection::connect().ok()?).ok()?;
    Some(
        state
            .windows
            .values()
            .filter_map(|window| window.pid)
            .collect(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwayNodeRole {
    Window,
    Container,
}

#[cfg(test)]
mod tests {
    use super::{SwayNode, SwayWorkspace, sway_state_of};
    use crate::compositor_window_ipc::compositor_window_state::CompositorWindowState;

    const SWAY_TREE: &str = r#"{
        "id": 1, "type": "root", "rect": {"x": 0, "y": 0, "width": 1280, "height": 800},
        "nodes": [
            {"id": 2, "type": "output", "name": "__i3", "rect": {"x": 0, "y": 0, "width": 0, "height": 0},
             "nodes": [{"id": 3, "type": "workspace", "name": "__i3_scratch", "rect": {"x": 0, "y": 0, "width": 0, "height": 0}}]},
            {"id": 4, "type": "output", "name": "Virtual-1", "rect": {"x": 0, "y": 0, "width": 1280, "height": 800},
             "nodes": [
                {"id": 10, "type": "workspace", "name": "1", "focus": [12, 11],
                 "rect": {"x": 0, "y": 0, "width": 1280, "height": 800},
                 "nodes": [
                    {"id": 11, "type": "con", "name": "editor", "app_id": "foot", "pid": 100,
                     "rect": {"x": 0, "y": 0, "width": 640, "height": 800},
                     "window_rect": {"x": 2, "y": 2, "width": 636, "height": 796}},
                    {"id": 12, "type": "con", "name": "browser", "app_id": null, "pid": 101, "focused": true,
                     "window_properties": {"class": "firefox"},
                     "rect": {"x": 640, "y": 0, "width": 640, "height": 800},
                     "window_rect": {"x": 2, "y": 2, "width": 636, "height": 796}}
                 ],
                 "floating_nodes": [
                    {"id": 13, "type": "floating_con", "name": "picker", "app_id": "zenity", "pid": 102,
                     "rect": {"x": 300, "y": 200, "width": 400, "height": 300}}
                 ]},
                {"id": 20, "type": "workspace", "name": "2: mail", "rect": {"x": 0, "y": 0, "width": 1280, "height": 800}}
             ]}
        ]
    }"#;

    const SWAY_WORKSPACES: &str = r#"[
        {"id": 10, "num": 1, "name": "1", "visible": true, "focused": true, "urgent": false, "output": "Virtual-1"},
        {"id": 20, "num": 2, "name": "2: mail", "visible": false, "focused": false, "urgent": true, "output": "Virtual-1"}
    ]"#;

    fn parsed_state() -> CompositorWindowState {
        let tree: SwayNode = serde_json::from_str(SWAY_TREE).unwrap();
        let workspaces: Vec<SwayWorkspace> = serde_json::from_str(SWAY_WORKSPACES).unwrap();
        sway_state_of(&workspaces, &tree)
    }

    #[test]
    fn windows_from_tiled_and_floating_containers_belong_to_their_workspace() {
        let state = parsed_state();

        let mut window_ids: Vec<u64> = state.windows.keys().copied().collect();
        window_ids.sort_unstable();
        assert_eq!(window_ids, [11, 12, 13]);
        assert!(
            state
                .windows
                .values()
                .all(|window| window.workspace_id == Some(10))
        );
        assert!(state.windows[&13].is_floating);
        assert_eq!(state.windows[&12].app_id.as_deref(), Some("firefox"));
        assert_eq!(state.windows[&11].layout.window_size, (636, 796));
        assert_eq!(
            state.windows[&12].layout.tile_position_in_view,
            Some((640.0, 0.0))
        );
    }

    #[test]
    fn workspaces_follow_sway_order_and_keep_only_given_names() {
        let state = parsed_state();

        let first = &state.workspaces[&10];
        let second = &state.workspaces[&20];
        assert_eq!((first.index, second.index), (1, 2));
        assert_eq!(first.name, None);
        assert_eq!(second.name.as_deref(), Some("2: mail"));
        assert!(first.is_focused && first.is_active);
        assert!(second.is_urgent && !second.is_active);
        assert_eq!(first.active_window_id, Some(12));
        assert!(!state.workspaces.contains_key(&3));
    }
}
