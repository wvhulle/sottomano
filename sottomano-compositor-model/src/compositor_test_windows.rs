use crate::{
    compositor_window_facts::{
        FocusTimestamp, Urgency, WindowAppId, WindowFocus, WindowPid, WindowTiling, WindowTitle,
        WindowWorkspace, WorkspaceActiveWindow, WorkspaceActivity, WorkspaceName, WorkspaceOutput,
    },
    compositor_window_state::{CompositorWindow, CompositorWindowLayout, CompositorWorkspace},
};

#[must_use]
pub fn unplaced_compositor_window(id: u64) -> CompositorWindow {
    CompositorWindow {
        id,
        title: WindowTitle::Untitled,
        app_id: WindowAppId::Unreported,
        pid: WindowPid::Unreported,
        workspace: WindowWorkspace::OnWorkspace(1),
        focus: WindowFocus::Unfocused,
        tiling: WindowTiling::Tiled,
        urgency: Urgency::NotUrgent,
        layout: CompositorWindowLayout::default(),
        focused_at_monotonic: FocusTimestamp::NeverFocused,
    }
}

#[must_use]
pub fn unnamed_compositor_workspace(id: u64) -> CompositorWorkspace {
    CompositorWorkspace {
        id,
        index: 1,
        name: WorkspaceName::Unnamed,
        output: WorkspaceOutput::Unassigned,
        urgency: Urgency::NotUrgent,
        activity: WorkspaceActivity::Focused,
        active_window: WorkspaceActiveWindow::NoWindow,
    }
}
