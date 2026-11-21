use super::{
    HyprlandClient, HyprlandMonitor, HyprlandWorkspace, TileViewPosition, WindowAppId, WindowFocus,
    WindowWorkspace, WorkspaceActiveWindow, WorkspaceActivity, WorkspaceName, compositor_state,
};
use crate::hyprland_workspace_identity::RegularHyprlandWorkspace;

const HYPRLAND_CLIENTS: &str = r#"[
    {"address": "0x5a1", "mapped": true, "at": [1290, 10], "size": [600, 400],
     "workspace": {"id": 2, "name": "web"}, "floating": false, "monitor": 1,
     "class": "firefox", "title": "Inbox", "pid": 300, "focusHistoryID": 0},
    {"address": "0x5a2", "mapped": true, "at": [10, 10], "size": [600, 400],
     "workspace": {"id": -98, "name": "special:scratch"}, "floating": true, "monitor": 0,
     "class": "", "title": "notes", "pid": 301, "focusHistoryID": 1}
]"#;

const HYPRLAND_WORKSPACES: &str = r#"[
    {"address": "2", "id": 2, "type": "normal", "name": "web", "monitor": "DP-2", "lastwindow": "0x5a1"},
    {"address": "Games", "type": "normal", "name": "Games", "monitor": "DP-1", "lastwindow": "0x0", "ispersistent": true},
    {"address": "1", "id": 1, "type": "normal", "name": "1", "monitor": "DP-1", "lastwindow": "0x0"},
    {"address": "special:scratch", "type": "special", "name": "scratch", "monitor": "DP-1", "lastwindow": "0x5a2"}
]"#;

const HYPRLAND_MONITORS: &str = r#"[
    {"id": 0, "name": "DP-1", "width": 1280, "height": 800, "refreshRate": 60.0, "x": 0, "y": 0,
     "activeWorkspace": {"id": 1, "name": "1"}, "specialWorkspace": {"address": "", "name": ""},
     "scale": 1.0, "focused": false},
    {"id": 1, "name": "DP-2", "width": 1920, "height": 1080, "refreshRate": 60.0, "x": 1280, "y": 0,
     "activeWorkspace": {"id": 2, "name": "web"}, "scale": 1.5, "focused": true}
]"#;

fn parsed_state() -> super::CompositorWindowState {
    let clients: Vec<HyprlandClient> = serde_json::from_str(HYPRLAND_CLIENTS).unwrap();
    let workspaces: Vec<HyprlandWorkspace> = serde_json::from_str(HYPRLAND_WORKSPACES).unwrap();
    let monitors: Vec<HyprlandMonitor> = serde_json::from_str(HYPRLAND_MONITORS).unwrap();
    compositor_state(&clients, &workspaces, &monitors)
}

#[test]
fn regular_workspaces_are_kept_with_their_monitor_flags() {
    let state = parsed_state();

    let games_key = RegularHyprlandWorkspace::Addressed {
        address: "Games".to_owned(),
    }
    .workspace_key();
    let mut workspace_ids: Vec<u64> = state.workspaces.keys().copied().collect();
    workspace_ids.sort_unstable();
    assert_eq!(workspace_ids, [1, 2, games_key]);
    let web = &state.workspaces[&2];
    assert_eq!(web.name, WorkspaceName::Named("web".to_owned()));
    assert_eq!(web.activity, WorkspaceActivity::Focused);
    assert_eq!(web.active_window, WorkspaceActiveWindow::Window(0x5a1));
    assert_eq!(state.workspaces[&1].name, WorkspaceName::Unnamed);
    assert_eq!(
        state.workspaces[&1].active_window,
        WorkspaceActiveWindow::NoWindow
    );
    assert_eq!(state.workspaces[&1].activity, WorkspaceActivity::Active);
    let games = &state.workspaces[&games_key];
    assert_eq!(games.name, WorkspaceName::Named("Games".to_owned()));
    assert_eq!(games.index, 2);
    assert_eq!(games.activity, WorkspaceActivity::Inactive);
}

#[test]
fn windows_carry_their_hex_address_and_position_on_their_monitor() {
    let state = parsed_state();

    let browser = &state.windows[&0x5a1];
    assert_eq!(browser.focus, WindowFocus::Focused);
    assert_eq!(browser.workspace, WindowWorkspace::OnWorkspace(2));
    assert_eq!(
        browser.layout.tile_position_in_view,
        TileViewPosition::InView {
            horizontal: 10.0,
            vertical: 10.0
        }
    );
    let scratch = &state.windows[&0x5a2];
    assert_eq!(scratch.workspace, WindowWorkspace::Unplaced);
    assert_eq!(scratch.app_id, WindowAppId::Unreported);
}
