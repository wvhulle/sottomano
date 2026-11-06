use niri_ipc::{Window, WindowLayout, Workspace};

#[must_use]
pub fn unplaced_niri_window(id: u64) -> Window {
    Window {
        id,
        title: None,
        app_id: None,
        pid: None,
        workspace_id: Some(1),
        is_focused: false,
        is_floating: false,
        is_urgent: false,
        layout: WindowLayout {
            pos_in_scrolling_layout: None,
            tile_size: (0.0, 0.0),
            window_size: (0, 0),
            tile_pos_in_workspace_view: None,
            window_offset_in_tile: (0.0, 0.0),
        },
        focus_timestamp: None,
    }
}

#[must_use]
pub fn unnamed_niri_workspace(id: u64) -> Workspace {
    Workspace {
        id,
        idx: 1,
        name: None,
        output: None,
        is_urgent: false,
        is_active: true,
        is_focused: true,
        active_window_id: None,
    }
}
