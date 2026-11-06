use std::fmt;

use crate::desktop_windows_apps::niri_action_requests::send_niri_request;

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
        follows_focus: bool,
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
}

#[derive(Debug, Clone)]
pub enum CompositorRequestFailure {
    Rejected { request: String, reason: String },
    Unreachable { request: String, reason: String },
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
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            CompositorRequestFailure::Rejected { reason, .. }
            | CompositorRequestFailure::Unreachable { reason, .. } => reason,
        }
    }

    #[must_use]
    pub fn request(&self) -> &str {
        match self {
            CompositorRequestFailure::Rejected { request, .. }
            | CompositorRequestFailure::Unreachable { request, .. } => request,
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

pub fn send_compositor_request(
    request: &CompositorRequest,
) -> Result<(), CompositorRequestFailure> {
    send_niri_request(request)
}

pub fn send_compositor_request_or_warn(request: &CompositorRequest) {
    if let Err(failure) = send_compositor_request(request) {
        tracing::warn!("{failure}");
    }
}
