use std::{
    collections::{BTreeSet, HashMap},
    io, mem,
};

use futures::{Stream, TryStreamExt, stream};
use serde::{Deserialize, de::DeserializeOwned};
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

use crate::{
    hyprland_socket_connection::{HyprlandEventSubscription, hyprland_request},
    hyprland_workspace_identity::{
        HyprlandWorkspaceIdentity, HyprlandWorkspaceReference, RegularHyprlandWorkspace,
    },
};

#[derive(Debug, Deserialize)]
struct HyprlandClient {
    address: String,
    #[serde(default = "mapped_by_default")]
    mapped: bool,
    at: [i32; 2],
    size: [i32; 2],
    workspace: HyprlandWorkspaceReference,
    #[serde(default)]
    floating: bool,
    monitor: i64,
    #[serde(default)]
    class: String,
    #[serde(default)]
    title: String,
    pid: i32,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: i64,
}

fn mapped_by_default() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct HyprlandWorkspace {
    #[serde(flatten)]
    reference: HyprlandWorkspaceReference,
    name: String,
    monitor: String,
    #[serde(rename = "lastwindow", default)]
    last_window: String,
}

#[derive(Debug, Deserialize)]
struct HyprlandMonitor {
    id: i64,
    name: String,
    #[serde(default)]
    make: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    serial: String,
    width: u16,
    height: u16,
    #[serde(rename = "refreshRate")]
    refresh_rate: f64,
    x: i32,
    y: i32,
    #[serde(rename = "activeWorkspace")]
    active_workspace: HyprlandWorkspaceReference,
    scale: f64,
    #[serde(default)]
    focused: bool,
}

#[derive(Debug, Deserialize)]
struct HyprlandVersion {
    tag: Option<String>,
    version: Option<String>,
}

async fn queried<Reply: DeserializeOwned>(command: &str) -> io::Result<Reply> {
    Ok(serde_json::from_str(&hyprland_request(command).await?)?)
}

pub(crate) fn parsed_window_id(address: &str) -> Option<u64> {
    let hex_digits = address.trim_start_matches("0x");
    if hex_digits.is_empty() {
        return None;
    }
    u64::from_str_radix(hex_digits, 16)
        .inspect_err(|parse_failure| {
            tracing::warn!("hyprland: invalid window address {address:?}: {parse_failure}");
        })
        .ok()
        .filter(|window_id| *window_id != 0)
}

fn regular_workspace_key(reference: &HyprlandWorkspaceReference) -> Option<u64> {
    match HyprlandWorkspaceIdentity::from(reference) {
        HyprlandWorkspaceIdentity::Regular(regular) => Some(regular.workspace_key()),
        HyprlandWorkspaceIdentity::Special | HyprlandWorkspaceIdentity::Unassigned => None,
    }
}

struct RegularHyprlandEntry<'listing> {
    regular: RegularHyprlandWorkspace,
    workspace: &'listing HyprlandWorkspace,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum WorkspaceListingOrder {
    ByNumber { id: u64 },
    AsListed,
}

impl From<&RegularHyprlandWorkspace> for WorkspaceListingOrder {
    fn from(regular: &RegularHyprlandWorkspace) -> Self {
        match regular {
            RegularHyprlandWorkspace::Numbered { id } => {
                WorkspaceListingOrder::ByNumber { id: *id }
            }
            RegularHyprlandWorkspace::Addressed { .. } => WorkspaceListingOrder::AsListed,
        }
    }
}

fn compositor_state(
    clients: &[HyprlandClient],
    workspaces: &[HyprlandWorkspace],
    monitors: &[HyprlandMonitor],
) -> CompositorWindowState {
    let monitor_named = |name: &str| monitors.iter().find(|monitor| monitor.name == name);
    let mut regular_workspaces: Vec<RegularHyprlandEntry> = workspaces
        .iter()
        .filter_map(
            |workspace| match HyprlandWorkspaceIdentity::from(&workspace.reference) {
                HyprlandWorkspaceIdentity::Regular(regular) => {
                    Some(RegularHyprlandEntry { regular, workspace })
                }
                HyprlandWorkspaceIdentity::Special | HyprlandWorkspaceIdentity::Unassigned => None,
            },
        )
        .collect();
    regular_workspaces.sort_by_key(|entry| WorkspaceListingOrder::from(&entry.regular));
    let mut next_index_by_monitor: HashMap<&str, u8> = HashMap::new();
    let compositor_workspaces: Vec<CompositorWorkspace> = regular_workspaces
        .into_iter()
        .map(|RegularHyprlandEntry { regular, workspace }| {
            let id = regular.workspace_key();
            let index = next_index_by_monitor
                .entry(workspace.monitor.as_str())
                .and_modify(|index| {
                    *index = index
                        .checked_add(1)
                        .expect("fewer than 255 workspaces share one monitor");
                })
                .or_insert(1);
            let activity = match monitor_named(&workspace.monitor) {
                Some(monitor) if regular_workspace_key(&monitor.active_workspace) == Some(id) => {
                    if monitor.focused {
                        WorkspaceActivity::Focused
                    } else {
                        WorkspaceActivity::Active
                    }
                }
                _ => WorkspaceActivity::Inactive,
            };
            CompositorWorkspace {
                id,
                index: *index,
                name: if workspace.name == id.to_string() {
                    WorkspaceName::Unnamed
                } else {
                    WorkspaceName::Named(workspace.name.clone())
                },
                output: WorkspaceOutput::Assigned(workspace.monitor.clone()),
                urgency: Urgency::NotUrgent,
                activity,
                active_window: parsed_window_id(&workspace.last_window).map_or(
                    WorkspaceActiveWindow::NoWindow,
                    WorkspaceActiveWindow::Window,
                ),
            }
        })
        .collect();
    let windows = clients
        .iter()
        .filter(|client| client.mapped)
        .filter_map(|client| {
            let monitor_origin = monitors
                .iter()
                .find(|monitor| monitor.id == client.monitor)
                .map_or((0, 0), |monitor| (monitor.x, monitor.y));
            Some(CompositorWindow {
                id: parsed_window_id(&client.address)?,
                title: WindowTitle::from(Some(client.title.clone())),
                app_id: WindowAppId::from(Some(client.class.clone())),
                pid: WindowPid::from(Some(client.pid)),
                workspace: regular_workspace_key(&client.workspace)
                    .map_or(WindowWorkspace::Unplaced, WindowWorkspace::OnWorkspace),
                focus: if client.focus_history_id == 0 {
                    WindowFocus::Focused
                } else {
                    WindowFocus::Unfocused
                },
                tiling: if client.floating {
                    WindowTiling::Floating
                } else {
                    WindowTiling::Tiled
                },
                urgency: Urgency::NotUrgent,
                layout: CompositorWindowLayout {
                    scrolling_position: ScrollingPosition::OutsideScrolling,
                    tile_size: (f64::from(client.size[0]), f64::from(client.size[1])),
                    window_size: (client.size[0], client.size[1]),
                    tile_position_in_view: TileViewPosition::InView {
                        horizontal: f64::from(client.at[0] - monitor_origin.0),
                        vertical: f64::from(client.at[1] - monitor_origin.1),
                    },
                    window_offset_in_tile: (0.0, 0.0),
                },
                focused_at_monotonic: FocusTimestamp::NeverFocused,
            })
        })
        .collect::<Vec<_>>();
    CompositorWindowState::indexed_by_id(compositor_workspaces, windows)
}

async fn read_hyprland_state() -> io::Result<CompositorWindowState> {
    let clients: Vec<HyprlandClient> = queried("j/clients").await?;
    let workspaces: Vec<HyprlandWorkspace> = queried("j/workspaces").await?;
    let monitors: Vec<HyprlandMonitor> = queried("j/monitors").await?;
    Ok(compositor_state(&clients, &workspaces, &monitors))
}

fn compositor_changes(event_name: &str) -> BTreeSet<CompositorChange> {
    match event_name {
        "openwindow" | "closewindow" | "movewindowv2" | "windowtitlev2" | "changefloatingmode"
        | "fullscreen" | "urgent" => [CompositorChange::Windows].into(),
        "activewindowv2" => [CompositorChange::Focus].into(),
        "workspacev2" | "focusedmonv2" => {
            [CompositorChange::Workspaces, CompositorChange::Focus].into()
        }
        "createworkspacev2" | "destroyworkspacev2" | "moveworkspacev2" | "renameworkspace"
        | "monitoraddedv2" | "monitorremoved" => [CompositorChange::Workspaces].into(),
        "configreloaded" => [CompositorChange::ConfigReload].into(),
        _ => BTreeSet::new(),
    }
}

struct HyprlandFollower {
    events: HyprlandEventSubscription,
    previous: CompositorWindowState,
    pending_changes: BTreeSet<CompositorChange>,
}

impl HyprlandFollower {
    async fn connected() -> io::Result<Self> {
        Ok(Self {
            events: HyprlandEventSubscription::connect().await?,
            previous: CompositorWindowState::default(),
            pending_changes: CompositorChange::ALL.into(),
        })
    }

    async fn next_update(mut self) -> io::Result<Option<(CompositorUpdate, Self)>> {
        while self.pending_changes.is_empty() {
            let Some(event_name) = self.events.next_event_name().await? else {
                return Ok(None);
            };
            self.pending_changes = compositor_changes(&event_name);
        }
        let state = read_hyprland_state()
            .await?
            .with_focus_times_from(&self.previous, monotonic_since_boot());
        self.previous = state.clone();
        let changes = mem::take(&mut self.pending_changes);
        Ok(Some((CompositorUpdate { state, changes }, self)))
    }
}

pub fn hyprland_compositor_updates() -> impl Stream<Item = io::Result<CompositorUpdate>> + Send {
    stream::once(async {
        HyprlandFollower::connected()
            .await
            .map(|follower| stream::try_unfold(follower, HyprlandFollower::next_update))
    })
    .try_flatten()
}

fn hyprland_output_facts(monitor: HyprlandMonitor) -> (String, OutputInfo) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let logical_size =
        |pixels: u16| (f64::from(pixels) / monitor.scale.max(f64::EPSILON)).round() as u32;
    let facts = OutputInfo {
        make: product_field(monitor.make),
        model: product_field(monitor.model),
        serial: product_field(monitor.serial),
        mode: OutputModeReport::Current(OutputMode {
            width: monitor.width,
            height: monitor.height,
            refresh_hz: monitor.refresh_rate,
        }),
        scale: monitor.scale,
        logical_width: logical_size(monitor.width),
        logical_height: logical_size(monitor.height),
    };
    (monitor.name, facts)
}

pub async fn hyprland_outputs() -> io::Result<HashMap<String, OutputInfo>> {
    let monitors: Vec<HyprlandMonitor> = queried("j/monitors").await?;
    Ok(monitors.into_iter().map(hyprland_output_facts).collect())
}

pub async fn hyprland_version() -> io::Result<Option<String>> {
    let version: HyprlandVersion = queried("j/version").await?;
    Ok(version.tag.or(version.version))
}

pub async fn hyprland_window_pids() -> io::Result<Vec<i32>> {
    let state = read_hyprland_state().await?;
    Ok(state
        .windows
        .values()
        .filter_map(|window| match window.pid {
            WindowPid::Reported(pid) => Some(pid),
            WindowPid::Unreported => None,
        })
        .collect())
}

#[cfg(test)]
mod hyprland_socket_events_tests;
