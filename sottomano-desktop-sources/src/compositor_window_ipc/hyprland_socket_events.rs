use std::{
    collections::{BTreeSet, HashMap},
    io,
};

use serde::{Deserialize, de::DeserializeOwned};
use tokio::sync::mpsc;

use crate::{
    compositor_window_ipc::{
        compositor_output_facts::{OutputInfo, OutputMode, product_field},
        compositor_window_state::{
            CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowLayout,
            CompositorWindowState, CompositorWorkspace,
        },
        hyprland_socket_connection::{HyprlandEvent, HyprlandEventSubscription, hyprland_request},
    },
    desktop_windows_apps::process_tree_inspection,
};

#[derive(Debug, Deserialize)]
struct HyprlandWorkspaceReference {
    id: i64,
}

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
    id: i64,
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

fn queried<Reply: DeserializeOwned>(command: &str) -> io::Result<Reply> {
    Ok(serde_json::from_str(&hyprland_request(command)?)?)
}

fn window_id_of(address: &str) -> Option<u64> {
    u64::from_str_radix(address.trim_start_matches("0x"), 16)
        .ok()
        .filter(|window_id| *window_id != 0)
}

fn regular_workspace_id(id: i64) -> Option<u64> {
    u64::try_from(id)
        .ok()
        .filter(|workspace_id| *workspace_id > 0)
}

fn hyprland_state_of(
    clients: &[HyprlandClient],
    workspaces: &[HyprlandWorkspace],
    monitors: &[HyprlandMonitor],
) -> CompositorWindowState {
    let monitor_named = |name: &str| monitors.iter().find(|monitor| monitor.name == name);
    let mut regular_workspaces: Vec<&HyprlandWorkspace> = workspaces
        .iter()
        .filter(|workspace| regular_workspace_id(workspace.id).is_some())
        .collect();
    regular_workspaces.sort_by_key(|workspace| workspace.id);
    let mut next_index_by_monitor: HashMap<&str, u8> = HashMap::new();
    let compositor_workspaces: Vec<CompositorWorkspace> = regular_workspaces
        .into_iter()
        .filter_map(|workspace| {
            let id = regular_workspace_id(workspace.id)?;
            let index = next_index_by_monitor
                .entry(workspace.monitor.as_str())
                .and_modify(|index| *index = index.saturating_add(1))
                .or_insert(1);
            let is_active = monitor_named(&workspace.monitor)
                .is_some_and(|monitor| monitor.active_workspace.id == workspace.id);
            Some(CompositorWorkspace {
                id,
                index: *index,
                name: (workspace.name != workspace.id.to_string()).then(|| workspace.name.clone()),
                output: Some(workspace.monitor.clone()),
                is_urgent: false,
                is_active,
                is_focused: is_active
                    && monitor_named(&workspace.monitor).is_some_and(|monitor| monitor.focused),
                active_window_id: window_id_of(&workspace.last_window),
            })
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
                id: window_id_of(&client.address)?,
                title: Some(client.title.clone()),
                app_id: Some(client.class.clone()).filter(|class| !class.is_empty()),
                pid: Some(client.pid).filter(|pid| *pid > 0),
                workspace_id: regular_workspace_id(client.workspace.id),
                is_focused: client.focus_history_id == 0,
                is_floating: client.floating,
                is_urgent: false,
                layout: CompositorWindowLayout {
                    scrolling_position: None,
                    tile_size: (f64::from(client.size[0]), f64::from(client.size[1])),
                    window_size: (client.size[0], client.size[1]),
                    tile_position_in_view: Some((
                        f64::from(client.at[0] - monitor_origin.0),
                        f64::from(client.at[1] - monitor_origin.1),
                    )),
                    window_offset_in_tile: (0.0, 0.0),
                },
                focused_at_monotonic: None,
            })
        })
        .collect::<Vec<_>>();
    CompositorWindowState::from_lists(compositor_workspaces, windows)
}

fn read_hyprland_state() -> io::Result<CompositorWindowState> {
    let clients: Vec<HyprlandClient> = queried("j/clients")?;
    let workspaces: Vec<HyprlandWorkspace> = queried("j/workspaces")?;
    let monitors: Vec<HyprlandMonitor> = queried("j/monitors")?;
    Ok(hyprland_state_of(&clients, &workspaces, &monitors))
}

fn hyprland_changes_of(event: &HyprlandEvent) -> BTreeSet<CompositorChange> {
    match event.name.as_str() {
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

pub fn follow_hyprland_events(sender: &mpsc::Sender<CompositorUpdate>) -> io::Result<()> {
    let mut subscription = HyprlandEventSubscription::connect()?;
    let mut previous = CompositorWindowState::default();
    let mut changes: BTreeSet<CompositorChange> = CompositorChange::ALL.into();
    loop {
        if !changes.is_empty() {
            let state = read_hyprland_state()?
                .with_focus_times_from(&previous, process_tree_inspection::monotonic());
            previous = state.clone();
            if sender
                .blocking_send(CompositorUpdate { state, changes })
                .is_err()
            {
                return Ok(());
            }
        }
        changes = hyprland_changes_of(&subscription.next_event()?);
    }
}

#[must_use]
pub fn hyprland_outputs() -> HashMap<String, OutputInfo> {
    let Ok(monitors) = queried::<Vec<HyprlandMonitor>>("j/monitors") else {
        return HashMap::new();
    };
    monitors
        .into_iter()
        .map(|monitor| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let logical_size =
                |pixels: u16| (f64::from(pixels) / monitor.scale.max(f64::EPSILON)).round() as u32;
            let facts = OutputInfo {
                make: product_field(monitor.make),
                model: product_field(monitor.model),
                serial: product_field(monitor.serial),
                mode: Some(OutputMode {
                    width: monitor.width,
                    height: monitor.height,
                    refresh_hz: monitor.refresh_rate,
                }),
                scale: monitor.scale,
                logical_width: logical_size(monitor.width),
                logical_height: logical_size(monitor.height),
            };
            (monitor.name, facts)
        })
        .collect()
}

#[must_use]
pub fn hyprland_version() -> Option<String> {
    let version: HyprlandVersion = queried("j/version").ok()?;
    version.tag.or(version.version)
}

#[must_use]
pub fn hyprland_window_pids() -> Option<Vec<i32>> {
    let state = read_hyprland_state().ok()?;
    Some(
        state
            .windows
            .values()
            .filter_map(|window| window.pid)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{HyprlandClient, HyprlandMonitor, HyprlandWorkspace, hyprland_state_of};

    const HYPRLAND_CLIENTS: &str = r#"[
        {"address": "0x5a1", "mapped": true, "at": [1290, 10], "size": [600, 400],
         "workspace": {"id": 2, "name": "web"}, "floating": false, "monitor": 1,
         "class": "firefox", "title": "Inbox", "pid": 300, "focusHistoryID": 0},
        {"address": "0x5a2", "mapped": true, "at": [10, 10], "size": [600, 400],
         "workspace": {"id": -98, "name": "special:scratch"}, "floating": true, "monitor": 0,
         "class": "", "title": "notes", "pid": 301, "focusHistoryID": 1}
    ]"#;

    const HYPRLAND_WORKSPACES: &str = r#"[
        {"id": 2, "name": "web", "monitor": "DP-2", "lastwindow": "0x5a1"},
        {"id": 1, "name": "1", "monitor": "DP-1", "lastwindow": "0x0"},
        {"id": -98, "name": "special:scratch", "monitor": "DP-1", "lastwindow": "0x5a2"}
    ]"#;

    const HYPRLAND_MONITORS: &str = r#"[
        {"id": 0, "name": "DP-1", "width": 1280, "height": 800, "refreshRate": 60.0, "x": 0, "y": 0,
         "activeWorkspace": {"id": 1, "name": "1"}, "scale": 1.0, "focused": false},
        {"id": 1, "name": "DP-2", "width": 1920, "height": 1080, "refreshRate": 60.0, "x": 1280, "y": 0,
         "activeWorkspace": {"id": 2, "name": "web"}, "scale": 1.5, "focused": true}
    ]"#;

    fn parsed_state() -> super::CompositorWindowState {
        let clients: Vec<HyprlandClient> = serde_json::from_str(HYPRLAND_CLIENTS).unwrap();
        let workspaces: Vec<HyprlandWorkspace> = serde_json::from_str(HYPRLAND_WORKSPACES).unwrap();
        let monitors: Vec<HyprlandMonitor> = serde_json::from_str(HYPRLAND_MONITORS).unwrap();
        hyprland_state_of(&clients, &workspaces, &monitors)
    }

    #[test]
    fn regular_workspaces_are_kept_with_their_monitor_flags() {
        let state = parsed_state();

        let mut workspace_ids: Vec<u64> = state.workspaces.keys().copied().collect();
        workspace_ids.sort_unstable();
        assert_eq!(workspace_ids, [1, 2]);
        let web = &state.workspaces[&2];
        assert_eq!(web.name.as_deref(), Some("web"));
        assert!(web.is_active && web.is_focused);
        assert_eq!(web.active_window_id, Some(0x5a1));
        assert_eq!(state.workspaces[&1].name, None);
        assert_eq!(state.workspaces[&1].active_window_id, None);
        assert!(!state.workspaces[&1].is_focused);
    }

    #[test]
    fn windows_carry_their_hex_address_and_position_on_their_monitor() {
        let state = parsed_state();

        let browser = &state.windows[&0x5a1];
        assert!(browser.is_focused);
        assert_eq!(browser.workspace_id, Some(2));
        assert_eq!(browser.layout.tile_position_in_view, Some((10.0, 10.0)));
        let scratch = &state.windows[&0x5a2];
        assert_eq!(scratch.workspace_id, None);
        assert_eq!(scratch.app_id, None);
    }
}
