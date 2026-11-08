use std::fmt;

use sottomano_desktop_sources::compositor_window_ipc::compositor_connection::{
    CompositorKind, missing_compositor_reason,
};
#[cfg(feature = "niri")]
use tokio::task::spawn_blocking;

#[cfg(feature = "hyprland")]
use crate::compositor_window_requests::hyprland_dispatch_requests::send_hyprland_request;
#[cfg(feature = "niri")]
use crate::compositor_window_requests::niri_action_requests::send_niri_request;
#[cfg(feature = "sway")]
use crate::compositor_window_requests::sway_command_requests::send_sway_request;

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
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            CompositorRequestFailure::Rejected { reason, .. }
            | CompositorRequestFailure::Unreachable { reason, .. } => reason,
            CompositorRequestFailure::Unsupported { compositor, .. } => compositor,
        }
    }

    #[must_use]
    pub fn request(&self) -> &str {
        match self {
            CompositorRequestFailure::Rejected { request, .. }
            | CompositorRequestFailure::Unreachable { request, .. }
            | CompositorRequestFailure::Unsupported { request, .. } => request,
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

pub async fn send_compositor_request(
    request: CompositorRequest,
) -> Result<(), CompositorRequestFailure> {
    match CompositorKind::detected() {
        #[cfg(feature = "niri")]
        Some(CompositorKind::Niri) => {
            let request_text = format!("{request:?}");
            spawn_blocking(move || send_niri_request(&request))
                .await
                .unwrap_or_else(|join_error| {
                    Err(CompositorRequestFailure::Unreachable {
                        request: request_text,
                        reason: join_error.to_string(),
                    })
                })
        }
        #[cfg(feature = "sway")]
        Some(CompositorKind::Sway) => send_sway_request(&request).await,
        #[cfg(feature = "hyprland")]
        Some(CompositorKind::Hyprland) => send_hyprland_request(&request).await,
        None => Err(CompositorRequestFailure::Unreachable {
            request: format!("{request:?}"),
            reason: missing_compositor_reason(),
        }),
    }
}

pub async fn send_compositor_request_or_warn(request: CompositorRequest) {
    if let Err(failure) = send_compositor_request(request).await {
        tracing::warn!("{failure}");
    }
}
