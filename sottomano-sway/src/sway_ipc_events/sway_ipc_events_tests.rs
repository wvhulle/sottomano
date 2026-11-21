use serde_json::{Value, json};
use sottomano_compositor_model::{
    compositor_window_facts::{
        TileViewPosition, Urgency, WindowAppId, WindowTiling, WindowWorkspace,
        WorkspaceActiveWindow, WorkspaceActivity, WorkspaceName,
    },
    compositor_window_state::CompositorWindowState,
};
use swayipc_types::{Node, Workspace};

use super::compositor_state;

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

fn rect(horizontal: i32, vertical: i32, width: i32, height: i32) -> Value {
    json!({"x": horizontal, "y": vertical, "width": width, "height": height})
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
    compositor_state(&sway_workspaces(), &sway_tree())
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
            .all(|window| window.workspace == WindowWorkspace::OnWorkspace(10))
    );
    assert_eq!(state.windows[&13].tiling, WindowTiling::Floating);
    assert_eq!(
        state.windows[&12].app_id,
        WindowAppId::Reported("firefox".to_owned())
    );
    assert_eq!(state.windows[&11].layout.window_size, (636, 796));
    assert_eq!(
        state.windows[&12].layout.tile_position_in_view,
        TileViewPosition::InView {
            horizontal: 640.0,
            vertical: 0.0
        }
    );
}

#[test]
fn workspaces_follow_sway_order_and_keep_only_given_names() {
    let state = parsed_state();

    let first = &state.workspaces[&10];
    let second = &state.workspaces[&20];
    assert_eq!((first.index, second.index), (1, 2));
    assert_eq!(first.name, WorkspaceName::Unnamed);
    assert_eq!(second.name, WorkspaceName::Named("2: mail".to_owned()));
    assert_eq!(first.activity, WorkspaceActivity::Focused);
    assert_eq!(second.urgency, Urgency::Urgent);
    assert_eq!(second.activity, WorkspaceActivity::Inactive);
    assert_eq!(first.active_window, WorkspaceActiveWindow::Window(12));
    assert!(!state.workspaces.contains_key(&3));
}
