use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompositorWindowLayout {
    pub scrolling_position: Option<(usize, usize)>,
    pub tile_size: (f64, f64),
    pub window_size: (i32, i32),
    pub tile_position_in_view: Option<(f64, f64)>,
    pub window_offset_in_tile: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorWindow {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub pid: Option<i32>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    pub is_floating: bool,
    pub is_urgent: bool,
    pub layout: CompositorWindowLayout,
    pub focused_at_monotonic: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorWorkspace {
    pub id: u64,
    pub index: u8,
    pub name: Option<String>,
    pub output: Option<String>,
    pub is_urgent: bool,
    pub is_active: bool,
    pub is_focused: bool,
    pub active_window_id: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct CompositorWindowState {
    pub workspaces: HashMap<u64, CompositorWorkspace>,
    pub windows: HashMap<u64, CompositorWindow>,
    pub overview_is_open: bool,
}

impl CompositorWindowState {
    #[must_use]
    pub fn from_lists(
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
            overview_is_open: false,
        }
    }

    #[must_use]
    pub fn with_focus_times_from(
        mut self,
        previous: &CompositorWindowState,
        now: Option<Duration>,
    ) -> Self {
        for window in self.windows.values_mut() {
            let earlier = previous.windows.get(&window.id);
            let newly_focused =
                window.is_focused && !earlier.is_some_and(|earlier| earlier.is_focused);
            window.focused_at_monotonic = if newly_focused {
                now
            } else {
                earlier.and_then(|earlier| earlier.focused_at_monotonic)
            };
        }
        self
    }

    #[must_use]
    pub fn focused_workspace(&self) -> Option<&CompositorWorkspace> {
        self.workspaces
            .values()
            .find(|workspace| workspace.is_focused)
    }

    #[must_use]
    pub fn focused_window(&self) -> Option<&CompositorWindow> {
        self.windows.values().find(|window| window.is_focused)
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

impl CompositorUpdate {
    #[must_use]
    pub fn changed(&self, change: CompositorChange) -> bool {
        self.changes.contains(&change)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CompositorWindow, CompositorWindowState};
    use crate::desktop_windows_apps::compositor_test_windows::unplaced_compositor_window;

    fn window_focused(id: u64, is_focused: bool) -> CompositorWindow {
        CompositorWindow {
            is_focused,
            ..unplaced_compositor_window(id)
        }
    }

    #[test]
    fn a_newly_focused_window_is_stamped_and_others_keep_their_stamp() {
        let first = CompositorWindowState::from_lists(
            [],
            [window_focused(1, true), window_focused(2, false)],
        )
        .with_focus_times_from(
            &CompositorWindowState::default(),
            Some(Duration::from_secs(10)),
        );
        let second = CompositorWindowState::from_lists(
            [],
            [window_focused(1, false), window_focused(2, true)],
        )
        .with_focus_times_from(&first, Some(Duration::from_secs(20)));

        assert_eq!(
            second.windows[&1].focused_at_monotonic,
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            second.windows[&2].focused_at_monotonic,
            Some(Duration::from_secs(20))
        );
    }

    #[test]
    fn a_window_that_stays_focused_keeps_its_first_stamp() {
        let first = CompositorWindowState::from_lists([], [window_focused(1, true)])
            .with_focus_times_from(
                &CompositorWindowState::default(),
                Some(Duration::from_secs(10)),
            );
        let second = CompositorWindowState::from_lists([], [window_focused(1, true)])
            .with_focus_times_from(&first, Some(Duration::from_secs(20)));

        assert_eq!(
            second.windows[&1].focused_at_monotonic,
            Some(Duration::from_secs(10))
        );
    }
}
