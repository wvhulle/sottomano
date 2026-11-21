use std::{error::Error, fmt};

use sottomano_toml_config::indicator_groups::gamepad_bindings::CompositorAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositorRequest {
    FocusWindow(u64),
    CloseWindow(u64),
    CloseFocusedWindow,
    FocusWorkspace(u64),
    MoveWorkspaceToIndex {
        workspace: u64,
        index: usize,
    },
    MoveNamedWorkspaceToIndex {
        name: String,
        index: usize,
    },
    MoveWindowToWorkspace {
        window: u64,
        workspace: u64,
        focus_follow: FocusFollow,
    },
    MoveFocusedColumnToIndex(usize),
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    MoveLeft,
    MoveRight,
    FocusWorkspaceUp,
    FocusWorkspaceDown,
    MoveFocusedWindowToWorkspaceUp,
    MoveFocusedWindowToWorkspaceDown,
    FocusPreviousWindow,
    ToggleOverview,
    MaximizeFocused,
    CenterFocused,
    Spawn(Vec<String>),
    PowerOffMonitors,
    Quit,
    ReloadConfig,
}

impl From<CompositorAction> for CompositorRequest {
    fn from(action: CompositorAction) -> Self {
        match action {
            CompositorAction::FocusLeft => Self::FocusLeft,
            CompositorAction::FocusRight => Self::FocusRight,
            CompositorAction::FocusUp => Self::FocusUp,
            CompositorAction::FocusDown => Self::FocusDown,
            CompositorAction::FocusWorkspaceUp => Self::FocusWorkspaceUp,
            CompositorAction::FocusWorkspaceDown => Self::FocusWorkspaceDown,
            CompositorAction::FocusPreviousWindow => Self::FocusPreviousWindow,
            CompositorAction::MoveLeft => Self::MoveLeft,
            CompositorAction::MoveRight => Self::MoveRight,
            CompositorAction::MoveWindowToWorkspaceUp => Self::MoveFocusedWindowToWorkspaceUp,
            CompositorAction::MoveWindowToWorkspaceDown => Self::MoveFocusedWindowToWorkspaceDown,
            CompositorAction::ToggleOverview => Self::ToggleOverview,
            CompositorAction::Maximize => Self::MaximizeFocused,
            CompositorAction::Center => Self::CenterFocused,
            CompositorAction::CloseWindow => Self::CloseFocusedWindow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusFollow {
    FollowWindow,
    StayOnCurrent,
}

#[derive(Debug, Clone)]
pub enum CompositorRequestFailure {
    Rejected {
        request: String,
        reason: String,
    },
    Unreachable {
        request: String,
        reason: String,
    },
    Unsupported {
        request: String,
        compositor: &'static str,
    },
    WorkspaceGone {
        request: String,
    },
}

impl CompositorRequestFailure {
    #[must_use]
    pub fn summary(&self) -> &'static str {
        match self {
            CompositorRequestFailure::Rejected { .. } => {
                "The compositor refused a request from the bar"
            }
            CompositorRequestFailure::Unreachable { .. } => {
                "The bar could not reach the compositor"
            }
            CompositorRequestFailure::Unsupported { .. } => {
                "The compositor has no equivalent for this request"
            }
            CompositorRequestFailure::WorkspaceGone { .. } => {
                "The workspace closed before the request reached the compositor"
            }
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            CompositorRequestFailure::Rejected { reason, .. }
            | CompositorRequestFailure::Unreachable { reason, .. } => reason,
            CompositorRequestFailure::Unsupported { compositor, .. } => compositor,
            CompositorRequestFailure::WorkspaceGone { .. } => "the compositor no longer lists it",
        }
    }

    #[must_use]
    pub fn request(&self) -> &str {
        match self {
            CompositorRequestFailure::Rejected { request, .. }
            | CompositorRequestFailure::Unreachable { request, .. }
            | CompositorRequestFailure::Unsupported { request, .. }
            | CompositorRequestFailure::WorkspaceGone { request } => request,
        }
    }
}

impl fmt::Display for CompositorRequestFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} ({})",
            self.summary(),
            self.reason(),
            self.request()
        )
    }
}

impl Error for CompositorRequestFailure {}
