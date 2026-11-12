use std::{
    collections::{BTreeSet, HashMap},
    mem,
    num::TryFromIntError,
};

use futures::{Stream, TryStreamExt, stream};
use swayipc_types::{
    CommandType, Event, EventType, Fallible, Node, NodeType, Output, Rect, Version, WindowChange,
    Workspace, WorkspaceChange,
};

use crate::{
    compositor_window_ipc::{
        compositor_output_facts::{OutputInfo, OutputMode, product_field},
        compositor_window_state::{
            CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowLayout,
            CompositorWindowState, CompositorWorkspace,
        },
        sway_ipc_connection::{SwayEventSubscription, SwayIpcConnection},
    },
    desktop_windows_apps::process_tree_inspection,
};

const SCRATCHPAD_OUTPUT: &str = "__i3";
const FOLLOWED_SWAY_EVENTS: [EventType; 4] = [
    EventType::Window,
    EventType::Workspace,
    EventType::Output,
    EventType::Shutdown,
];

fn sway_id(id: i64) -> Option<u64> {
    u64::try_from(id)
        .inspect_err(|range_failure| tracing::warn!("sway: negative node id {id}: {range_failure}"))
        .ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwayNodeRole {
    Window,
    Container,
}

fn node_role(node: &Node) -> SwayNodeRole {
    if matches!(node.node_type, NodeType::Con | NodeType::FloatingCon)
        && node.nodes.is_empty()
        && node.floating_nodes.is_empty()
        && (node.pid.is_some() || node.app_id.is_some() || node.window_properties.is_some())
    {
        SwayNodeRole::Window
    } else {
        SwayNodeRole::Container
    }
}

fn focused_window(node: &Node) -> Option<u64> {
    if node_role(node) == SwayNodeRole::Window {
        return sway_id(node.id);
    }
    let focused_child_id = node.focus.first()?;
    focused_window(
        node.nodes
            .iter()
            .chain(&node.floating_nodes)
            .find(|child| child.id == *focused_child_id)?,
    )
}

fn window_app_id(node: &Node) -> Option<String> {
    node.app_id
        .clone()
        .or_else(|| node.window_properties.as_ref()?.class.clone())
}

struct PlacedSwayWorkspace<'tree> {
    node: &'tree Node,
    output_rect: Rect,
}

fn placed_workspaces(tree: &Node) -> HashMap<i64, PlacedSwayWorkspace<'_>> {
    tree.nodes
        .iter()
        .filter(|output| output.name.as_deref() != Some(SCRATCHPAD_OUTPUT))
        .flat_map(|output| {
            output
                .nodes
                .iter()
                .filter(|workspace| workspace.node_type == NodeType::Workspace)
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

enum WindowPlacement {
    Floating,
    Tiled,
}

fn collect_windows(
    node: &Node,
    workspace_id: u64,
    output_rect: Rect,
    placement: &WindowPlacement,
    windows: &mut Vec<CompositorWindow>,
) {
    if node_role(node) == SwayNodeRole::Window {
        windows.extend(sway_window_of(node, workspace_id, output_rect, placement));
        return;
    }
    for child in &node.nodes {
        collect_windows(child, workspace_id, output_rect, placement, windows);
    }
    for child in &node.floating_nodes {
        collect_windows(
            child,
            workspace_id,
            output_rect,
            &WindowPlacement::Floating,
            windows,
        );
    }
}

fn sway_window_of(
    node: &Node,
    workspace_id: u64,
    output_rect: Rect,
    placement: &WindowPlacement,
) -> Option<CompositorWindow> {
    Some(CompositorWindow {
        id: sway_id(node.id)?,
        title: node.name.clone(),
        app_id: window_app_id(node),
        pid: node.pid,
        workspace_id: Some(workspace_id),
        is_focused: node.focused,
        is_floating: matches!(placement, WindowPlacement::Floating),
        is_urgent: node.urgent,
        layout: CompositorWindowLayout {
            scrolling_position: None,
            tile_size: (f64::from(node.rect.width), f64::from(node.rect.height)),
            window_size: (node.window_rect.width, node.window_rect.height),
            tile_position_in_view: Some((
                f64::from(node.rect.x - output_rect.x),
                f64::from(node.rect.y - output_rect.y),
            )),
            window_offset_in_tile: (f64::from(node.window_rect.x), f64::from(node.window_rect.y)),
        },
        focused_at_monotonic: None,
    })
}

fn given_workspace_name(workspace: &Workspace) -> Option<String> {
    (workspace.name != workspace.num.to_string()).then(|| workspace.name.clone())
}

fn sway_state_of(workspaces: &[Workspace], tree: &Node) -> CompositorWindowState {
    let placed = placed_workspaces(tree);
    let mut next_index_by_output: HashMap<&str, u8> = HashMap::new();
    let compositor_workspaces: Vec<CompositorWorkspace> = workspaces
        .iter()
        .filter_map(|workspace| {
            let placed_workspace = placed.get(&workspace.id)?;
            let index = next_index_by_output
                .entry(workspace.output.as_str())
                .and_modify(|index| *index = index.saturating_add(1))
                .or_insert(1);
            Some(CompositorWorkspace {
                id: sway_id(workspace.id)?,
                index: *index,
                name: given_workspace_name(workspace),
                output: Some(workspace.output.clone()),
                is_urgent: workspace.urgent,
                is_active: workspace.visible,
                is_focused: workspace.focused,
                active_window_id: focused_window(placed_workspace.node),
            })
        })
        .collect();
    let mut windows = Vec::new();
    for (workspace_id, placed_workspace) in &placed {
        if let Some(workspace_id) = sway_id(*workspace_id) {
            collect_windows(
                placed_workspace.node,
                workspace_id,
                placed_workspace.output_rect,
                &WindowPlacement::Tiled,
                &mut windows,
            );
        }
    }
    CompositorWindowState::from_lists(compositor_workspaces, windows)
}

async fn read_sway_state(queries: &mut SwayIpcConnection) -> Fallible<CompositorWindowState> {
    let workspaces: Vec<Workspace> = queries.request(CommandType::GetWorkspaces, "").await?;
    let tree: Node = queries.request(CommandType::GetTree, "").await?;
    Ok(sway_state_of(&workspaces, &tree))
}

fn sway_changes_of(event: &Event) -> BTreeSet<CompositorChange> {
    match event {
        Event::Window(window_event) if window_event.change == WindowChange::Focus => {
            [CompositorChange::Focus].into()
        }
        Event::Window(_) => [CompositorChange::Windows].into(),
        Event::Workspace(workspace_event) if workspace_event.change == WorkspaceChange::Focus => {
            [CompositorChange::Workspaces, CompositorChange::Focus].into()
        }
        Event::Workspace(_) | Event::Output(_) => [CompositorChange::Workspaces].into(),
        _ => BTreeSet::new(),
    }
}

struct SwayFollower {
    subscription: SwayEventSubscription,
    queries: SwayIpcConnection,
    previous: CompositorWindowState,
    pending_changes: BTreeSet<CompositorChange>,
}

impl SwayFollower {
    async fn connected() -> Fallible<Self> {
        Ok(Self {
            subscription: SwayIpcConnection::connect()
                .await?
                .subscribe(&FOLLOWED_SWAY_EVENTS)
                .await?,
            queries: SwayIpcConnection::connect().await?,
            previous: CompositorWindowState::default(),
            pending_changes: CompositorChange::ALL.into(),
        })
    }

    async fn next_update(mut self) -> Fallible<Option<(CompositorUpdate, Self)>> {
        while self.pending_changes.is_empty() {
            let event = self.subscription.next_event().await?;
            if matches!(event, Event::Shutdown(_)) {
                return Ok(None);
            }
            self.pending_changes = sway_changes_of(&event);
        }
        let state = read_sway_state(&mut self.queries)
            .await?
            .with_focus_times_from(&self.previous, process_tree_inspection::monotonic());
        self.previous = state.clone();
        let changes = mem::take(&mut self.pending_changes);
        Ok(Some((CompositorUpdate { state, changes }, self)))
    }
}

pub fn sway_compositor_updates() -> impl Stream<Item = Fallible<CompositorUpdate>> + Send {
    stream::once(async {
        SwayFollower::connected()
            .await
            .map(|follower| stream::try_unfold(follower, SwayFollower::next_update))
    })
    .try_flatten()
}

fn sway_output_facts(output: Output) -> (String, OutputInfo) {
    let facts = OutputInfo {
        make: product_field(output.make),
        model: product_field(output.model),
        serial: product_field(output.serial),
        mode: output.current_mode.and_then(|mode| {
            let log_range_failure = |range_failure: &TryFromIntError| {
                tracing::warn!(
                    "sway: output {} mode {}x{} out of range: {range_failure}",
                    output.name,
                    mode.width,
                    mode.height
                );
            };
            Some(OutputMode {
                width: u16::try_from(mode.width)
                    .inspect_err(&log_range_failure)
                    .ok()?,
                height: u16::try_from(mode.height)
                    .inspect_err(&log_range_failure)
                    .ok()?,
                refresh_hz: f64::from(mode.refresh) / 1000.0,
            })
        }),
        scale: output.scale.unwrap_or(1.0),
        logical_width: u32::try_from(output.rect.width).unwrap_or(0),
        logical_height: u32::try_from(output.rect.height).unwrap_or(0),
    };
    (output.name, facts)
}

pub async fn sway_outputs() -> Fallible<HashMap<String, OutputInfo>> {
    let outputs: Vec<Output> = SwayIpcConnection::connect()
        .await?
        .request(CommandType::GetOutputs, "")
        .await?;
    Ok(outputs
        .into_iter()
        .filter(|output| output.active)
        .map(sway_output_facts)
        .collect())
}

pub async fn sway_version() -> Fallible<String> {
    let version: Version = SwayIpcConnection::connect()
        .await?
        .request(CommandType::GetVersion, "")
        .await?;
    Ok(version.human_readable)
}

pub async fn sway_window_pids() -> Fallible<Vec<i32>> {
    let state = read_sway_state(&mut SwayIpcConnection::connect().await?).await?;
    Ok(state
        .windows
        .values()
        .filter_map(|window| window.pid)
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use swayipc_types::{Node, Workspace};

    use super::sway_state_of;
    use crate::compositor_window_ipc::compositor_window_state::CompositorWindowState;

    fn node(fields: &Value) -> Value {
        let mut node = json!({
            "id": 0, "type": "con", "border": "none", "current_border_width": 0,
            "layout": "none", "orientation": "none",
            "rect": {"x": 0, "y": 0, "width": 0, "height": 0},
            "window_rect": {"x": 0, "y": 0, "width": 0, "height": 0},
            "deco_rect": {"x": 0, "y": 0, "width": 0, "height": 0},
            "geometry": {"x": 0, "y": 0, "width": 0, "height": 0},
            "urgent": false, "focused": false, "focus": [], "nodes": [],
            "floating_nodes": [], "sticky": false
        });
        for (key, value) in fields.as_object().into_iter().flatten() {
            node[key] = value.clone();
        }
        node
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Value {
        json!({"x": x, "y": y, "width": width, "height": height})
    }

    fn sway_tree() -> Node {
        let tree = node(&json!({
            "id": 1, "type": "root",
            "nodes": [
                node(&json!({"id": 2, "type": "output", "name": "__i3",
                    "nodes": [node(&json!({"id": 3, "type": "workspace", "name": "__i3_scratch"}))]})),
                node(&json!({"id": 4, "type": "output", "name": "Virtual-1",
                    "rect": rect(0, 0, 1280, 800),
                    "nodes": [
                        node(&json!({"id": 10, "type": "workspace", "name": "1", "focus": [12, 11],
                            "nodes": [
                                node(&json!({"id": 11, "name": "editor", "app_id": "foot", "pid": 100,
                                    "rect": rect(0, 0, 640, 800), "window_rect": rect(2, 2, 636, 796)})),
                                node(&json!({"id": 12, "name": "browser", "pid": 101, "focused": true,
                                    "window_properties": {"class": "firefox"},
                                    "rect": rect(640, 0, 640, 800), "window_rect": rect(2, 2, 636, 796)}))
                            ],
                            "floating_nodes": [
                                node(&json!({"id": 13, "type": "floating_con", "name": "picker",
                                    "app_id": "zenity", "pid": 102, "rect": rect(300, 200, 400, 300)}))
                            ]})),
                        node(&json!({"id": 20, "type": "workspace", "name": "2: mail"}))
                    ]}))
            ]
        }));
        serde_json::from_value(tree).unwrap()
    }

    fn sway_workspaces() -> Vec<Workspace> {
        let workspace = |id: i64, num: i32, name: &str, visible: bool, urgent: bool| {
            json!({"id": id, "num": num, "name": name, "visible": visible, "focused": visible,
                "urgent": urgent, "output": "Virtual-1", "rect": rect(0, 0, 1280, 800)})
        };
        serde_json::from_value(json!([
            workspace(10, 1, "1", true, false),
            workspace(20, 2, "2: mail", false, true)
        ]))
        .unwrap()
    }

    fn parsed_state() -> CompositorWindowState {
        sway_state_of(&sway_workspaces(), &sway_tree())
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
