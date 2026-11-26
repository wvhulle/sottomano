use std::{
    collections::{BTreeSet, HashMap},
    mem,
    num::TryFromIntError,
};

use futures::{Stream, TryStreamExt, stream};
use sottomano_compositor_model::{
    compositor_output_facts::{OutputInfo, OutputMode, OutputModeReport, product_field},
    compositor_window_facts::{
        FocusTimestamp, ScrollingPosition, TileViewPosition, Urgency, WindowAppId, WindowFocus,
        WindowPid, WindowTiling, WindowTitle, WindowWorkspace, WorkspaceActiveWindow,
        WorkspaceActivity, WorkspaceName, WorkspaceOutput,
    },
    compositor_window_state::{
        CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowLayout,
        CompositorWindowState, CompositorWorkspace,
    },
    monotonic_clock::monotonic_since_boot,
};
use swayipc_types::{
    CommandType, Event, EventType, Fallible, Node, NodeType, Output, Rect, Version, WindowChange,
    Workspace, WorkspaceChange,
};

use crate::sway_ipc_connection::{SwayEventSubscription, SwayIpcConnection};

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

fn window_app_id(node: &Node) -> WindowAppId {
    WindowAppId::from(
        node.app_id
            .clone()
            .or_else(|| node.window_properties.as_ref()?.class.clone()),
    )
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

fn collect_windows(
    node: &Node,
    workspace_id: u64,
    output_rect: Rect,
    tiling: WindowTiling,
    windows: &mut Vec<CompositorWindow>,
) {
    if node_role(node) == SwayNodeRole::Window {
        windows.extend(compositor_window(node, workspace_id, output_rect, tiling));
        return;
    }
    for child in &node.nodes {
        collect_windows(child, workspace_id, output_rect, tiling, windows);
    }
    for child in &node.floating_nodes {
        collect_windows(
            child,
            workspace_id,
            output_rect,
            WindowTiling::Floating,
            windows,
        );
    }
}

fn compositor_window(
    node: &Node,
    workspace_id: u64,
    output_rect: Rect,
    tiling: WindowTiling,
) -> Option<CompositorWindow> {
    Some(CompositorWindow {
        id: sway_id(node.id)?,
        title: WindowTitle::from(node.name.clone()),
        app_id: window_app_id(node),
        pid: WindowPid::from(node.pid),
        workspace: WindowWorkspace::OnWorkspace(workspace_id),
        focus: if node.focused {
            WindowFocus::Focused
        } else {
            WindowFocus::Unfocused
        },
        tiling,
        urgency: if node.urgent {
            Urgency::Urgent
        } else {
            Urgency::NotUrgent
        },
        layout: CompositorWindowLayout {
            scrolling_position: ScrollingPosition::OutsideScrolling,
            tile_size: (f64::from(node.rect.width), f64::from(node.rect.height)),
            window_size: (node.window_rect.width, node.window_rect.height),
            tile_position_in_view: TileViewPosition::InView {
                horizontal: f64::from(node.rect.x - output_rect.x),
                vertical: f64::from(node.rect.y - output_rect.y),
            },
            window_offset_in_tile: (f64::from(node.window_rect.x), f64::from(node.window_rect.y)),
        },
        focused_at_monotonic: FocusTimestamp::NeverFocused,
    })
}

fn given_workspace_name(workspace: &Workspace) -> WorkspaceName {
    if workspace.name == workspace.num.to_string() {
        WorkspaceName::Unnamed
    } else {
        WorkspaceName::Named(workspace.name.clone())
    }
}

fn compositor_state(workspaces: &[Workspace], tree: &Node) -> CompositorWindowState {
    let placed = placed_workspaces(tree);
    let mut next_index_by_output: HashMap<&str, u8> = HashMap::new();
    let compositor_workspaces: Vec<CompositorWorkspace> = workspaces
        .iter()
        .filter_map(|workspace| {
            let placed_workspace = placed.get(&workspace.id)?;
            let index = next_index_by_output
                .entry(workspace.output.as_str())
                .and_modify(|index| {
                    *index = index
                        .checked_add(1)
                        .expect("fewer than 255 workspaces share one output");
                })
                .or_insert(1);
            Some(CompositorWorkspace {
                id: sway_id(workspace.id)?,
                index: *index,
                name: given_workspace_name(workspace),
                output: WorkspaceOutput::Assigned(workspace.output.clone()),
                urgency: if workspace.urgent {
                    Urgency::Urgent
                } else {
                    Urgency::NotUrgent
                },
                activity: match (workspace.focused, workspace.visible) {
                    (true, _) => WorkspaceActivity::Focused,
                    (false, true) => WorkspaceActivity::Active,
                    (false, false) => WorkspaceActivity::Inactive,
                },
                active_window: focused_window(placed_workspace.node).map_or(
                    WorkspaceActiveWindow::NoWindow,
                    WorkspaceActiveWindow::Window,
                ),
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
                WindowTiling::Tiled,
                &mut windows,
            );
        }
    }
    CompositorWindowState::indexed_by_id(compositor_workspaces, windows)
}

async fn read_sway_state(queries: &mut SwayIpcConnection) -> Fallible<CompositorWindowState> {
    let workspaces: Vec<Workspace> = queries.request(CommandType::GetWorkspaces, "").await?;
    let tree: Node = queries.request(CommandType::GetTree, "").await?;
    Ok(compositor_state(&workspaces, &tree))
}

fn compositor_changes(event: &Event) -> BTreeSet<CompositorChange> {
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
            self.pending_changes = compositor_changes(&event);
        }
        let state = read_sway_state(&mut self.queries)
            .await?
            .with_focus_times_from(&self.previous, monotonic_since_boot());
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

fn sway_current_mode(output: &Output) -> OutputModeReport {
    let Some(mode) = output.current_mode else {
        return OutputModeReport::Unreported;
    };
    let log_range_failure = |range_failure: &TryFromIntError| {
        tracing::warn!(
            "sway: output {} mode {}x{} out of range: {range_failure}",
            output.name,
            mode.width,
            mode.height
        );
    };
    let width = u16::try_from(mode.width).inspect_err(&log_range_failure);
    let height = u16::try_from(mode.height).inspect_err(&log_range_failure);
    let (Ok(width), Ok(height)) = (width, height) else {
        return OutputModeReport::Unreported;
    };
    OutputModeReport::Current(OutputMode {
        width,
        height,
        refresh_hz: f64::from(mode.refresh) / 1000.0,
    })
}

fn sway_output_facts(output: Output) -> (String, OutputInfo) {
    let mode = sway_current_mode(&output);
    let facts = OutputInfo {
        make: product_field(output.make),
        model: product_field(output.model),
        serial: product_field(output.serial),
        mode,
        scale: output.scale.unwrap_or(1.0),
        logical_width: output_side(&output.name, output.rect.width),
        logical_height: output_side(&output.name, output.rect.height),
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
        .filter_map(|window| match window.pid {
            WindowPid::Reported(pid) => Some(pid),
            WindowPid::Unreported => None,
        })
        .collect())
}

fn output_side(output_name: &str, reported_side: i32) -> u32 {
    u32::try_from(reported_side).unwrap_or_else(|negative_side| {
        tracing::warn!("sway: output {output_name} reports side {reported_side}: {negative_side}");
        0
    })
}

#[cfg(test)]
mod sway_ipc_events_tests;
