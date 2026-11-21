use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowTitle {
    Untitled,
    Titled(String),
}

impl From<Option<String>> for WindowTitle {
    fn from(reported_title: Option<String>) -> Self {
        match reported_title {
            Some(title) if !title.is_empty() => WindowTitle::Titled(title),
            _ => WindowTitle::Untitled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowAppId {
    Unreported,
    Reported(String),
}

impl From<Option<String>> for WindowAppId {
    fn from(reported_app_id: Option<String>) -> Self {
        match reported_app_id {
            Some(app_id) if !app_id.is_empty() => WindowAppId::Reported(app_id),
            _ => WindowAppId::Unreported,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowPid {
    Unreported,
    Reported(i32),
}

impl From<Option<i32>> for WindowPid {
    fn from(reported_pid: Option<i32>) -> Self {
        match reported_pid {
            Some(pid) if pid > 0 => WindowPid::Reported(pid),
            _ => WindowPid::Unreported,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowWorkspace {
    Unplaced,
    OnWorkspace(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowFocus {
    Focused,
    Unfocused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowTiling {
    Tiled,
    Floating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Urgency {
    Urgent,
    NotUrgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FocusTimestamp {
    NeverFocused,
    FocusedAt(Duration),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScrollingPosition {
    #[default]
    OutsideScrolling,
    Tile {
        column: usize,
        tile: usize,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum TileViewPosition {
    #[default]
    OutsideView,
    InView {
        horizontal: f64,
        vertical: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkspaceName {
    Unnamed,
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkspaceOutput {
    Unassigned,
    Assigned(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspaceActivity {
    Inactive,
    Active,
    Focused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspaceActiveWindow {
    NoWindow,
    Window(u64),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverviewVisibility {
    #[default]
    Closed,
    Open,
}
