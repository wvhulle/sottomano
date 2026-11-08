use crate::compositor_window_ipc::compositor_window_state::{
    CompositorWindow, CompositorWindowLayout, CompositorWorkspace,
};

#[must_use]
pub fn unplaced_compositor_window(id: u64) -> CompositorWindow {
    CompositorWindow {
        id,
        title: None,
        app_id: None,
        pid: None,
        workspace_id: Some(1),
        is_focused: false,
        is_floating: false,
        is_urgent: false,
        layout: CompositorWindowLayout::default(),
        focused_at_monotonic: None,
    }
}

#[must_use]
pub fn unnamed_compositor_workspace(id: u64) -> CompositorWorkspace {
    CompositorWorkspace {
        id,
        index: 1,
        name: None,
        output: None,
        is_urgent: false,
        is_active: true,
        is_focused: true,
        active_window_id: None,
    }
}
