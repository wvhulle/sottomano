use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use crate::compositor_window_facts::{
    FocusTimestamp, OverviewVisibility, ScrollingPosition, TileViewPosition, Urgency, WindowAppId,
    WindowFocus, WindowPid, WindowTiling, WindowTitle, WindowWorkspace, WorkspaceActiveWindow,
    WorkspaceActivity, WorkspaceName, WorkspaceOutput,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompositorWindowLayout {
    pub scrolling_position: ScrollingPosition,
    pub tile_size: (f64, f64),
    pub window_size: (i32, i32),
    pub tile_position_in_view: TileViewPosition,
    pub window_offset_in_tile: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorWindow {
    pub id: u64,
    pub title: WindowTitle,
    pub app_id: WindowAppId,
    pub pid: WindowPid,
    pub workspace: WindowWorkspace,
    pub focus: WindowFocus,
    pub tiling: WindowTiling,
    pub urgency: Urgency,
    pub layout: CompositorWindowLayout,
    pub focused_at_monotonic: FocusTimestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorWorkspace {
    pub id: u64,
    pub index: u8,
    pub name: WorkspaceName,
    pub output: WorkspaceOutput,
    pub urgency: Urgency,
    pub activity: WorkspaceActivity,
    pub active_window: WorkspaceActiveWindow,
}

#[derive(Debug, Clone, Default)]
pub struct CompositorWindowState {
    pub workspaces: HashMap<u64, CompositorWorkspace>,
    pub windows: HashMap<u64, CompositorWindow>,
    pub overview: OverviewVisibility,
}

impl CompositorWindowState {
    #[must_use]
    pub fn indexed_by_id(
        workspaces: impl IntoIterator<Item = CompositorWorkspace>,
        windows: impl IntoIterator<Item = CompositorWindow>,
    ) -> Self {
        Self {
            workspaces: workspaces
                .into_iter()
                .map(|workspace| (workspace.id, workspace))
                .collect(),
            windows: windows
                .into_iter()
                .map(|window| (window.id, window))
                .collect(),
            overview: OverviewVisibility::Closed,
        }
    }

    #[must_use]
    pub fn with_focus_times_from(
        mut self,
        previous: &CompositorWindowState,
        now: Duration,
    ) -> Self {
        for window in self.windows.values_mut() {
            let earlier = previous.windows.get(&window.id);
            let earlier_focus = earlier.map_or(WindowFocus::Unfocused, |earlier| earlier.focus);
            window.focused_at_monotonic = match (window.focus, earlier_focus) {
                (WindowFocus::Focused, WindowFocus::Unfocused) => FocusTimestamp::FocusedAt(now),
                _ => earlier.map_or(FocusTimestamp::NeverFocused, |earlier| {
                    earlier.focused_at_monotonic
                }),
            };
        }
        self
    }

    #[must_use]
    pub fn focused_workspace(&self) -> Option<&CompositorWorkspace> {
        self.workspaces
            .values()
            .find(|workspace| workspace.activity == WorkspaceActivity::Focused)
    }

    #[must_use]
    pub fn focused_window(&self) -> Option<&CompositorWindow> {
        self.windows
            .values()
            .find(|window| window.focus == WindowFocus::Focused)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompositorChange {
    Windows,
    Workspaces,
    Focus,
    ConfigReload,
}

impl CompositorChange {
    pub const ALL: [CompositorChange; 4] = [
        CompositorChange::Windows,
        CompositorChange::Workspaces,
        CompositorChange::Focus,
        CompositorChange::ConfigReload,
    ];
}

#[derive(Debug, Clone)]
pub struct CompositorUpdate {
    pub state: CompositorWindowState,
    pub changes: BTreeSet<CompositorChange>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CompositorWindow, CompositorWindowState};
    use crate::{
        compositor_test_windows::unplaced_compositor_window,
        compositor_window_facts::{FocusTimestamp, WindowFocus},
    };

    fn window_with_focus(id: u64, focus: WindowFocus) -> CompositorWindow {
        CompositorWindow {
            focus,
            ..unplaced_compositor_window(id)
        }
    }

    #[test]
    fn a_newly_focused_window_is_stamped_and_others_keep_their_stamp() {
        let first = CompositorWindowState::indexed_by_id(
            [],
            [
                window_with_focus(1, WindowFocus::Focused),
                window_with_focus(2, WindowFocus::Unfocused),
            ],
        )
        .with_focus_times_from(&CompositorWindowState::default(), Duration::from_secs(10));
        let second = CompositorWindowState::indexed_by_id(
            [],
            [
                window_with_focus(1, WindowFocus::Unfocused),
                window_with_focus(2, WindowFocus::Focused),
            ],
        )
        .with_focus_times_from(&first, Duration::from_secs(20));

        assert_eq!(
            second.windows[&1].focused_at_monotonic,
            FocusTimestamp::FocusedAt(Duration::from_secs(10))
        );
        assert_eq!(
            second.windows[&2].focused_at_monotonic,
            FocusTimestamp::FocusedAt(Duration::from_secs(20))
        );
    }

    #[test]
    fn a_window_that_stays_focused_keeps_its_first_stamp() {
        let first =
            CompositorWindowState::indexed_by_id([], [window_with_focus(1, WindowFocus::Focused)])
                .with_focus_times_from(&CompositorWindowState::default(), Duration::from_secs(10));
        let second =
            CompositorWindowState::indexed_by_id([], [window_with_focus(1, WindowFocus::Focused)])
                .with_focus_times_from(&first, Duration::from_secs(20));

        assert_eq!(
            second.windows[&1].focused_at_monotonic,
            FocusTimestamp::FocusedAt(Duration::from_secs(10))
        );
    }
}
