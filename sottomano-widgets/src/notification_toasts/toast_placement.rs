use corredo::transitions::MotionState;
use iced::{Element, Rectangle};
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::NotificationData,
    desktop_windows_apps::window_edge_placement::{Placement, WindowOwner},
};
use sottomano_toml_config::{
    Config,
    screen_edges::{HEdge, VEdge},
};

use crate::{
    indicator_groups::overlay_settings_group::ToastArrangement,
    notification_toasts::NotificationSettings, toast_messages::NotificationMessage,
    widgets::typing_under_cards::viewed_window_bounds,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastAnchor {
    Edge { vertical: VEdge, horizontal: HEdge },
    Window { id: u64, bounds: Rectangle },
}

pub struct ToastPlacement {
    pub window: ToastWindow,
    pub window_focus: AnchoredWindowFocus,
    pub output: Option<String>,
    pub anchor: ToastAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchoredWindowFocus {
    Focused,
    OnFocusedWorkspace,
    Elsewhere,
    Unknown,
}

impl AnchoredWindowFocus {
    #[must_use]
    pub fn makes_move_pointless(self) -> bool {
        matches!(self, Self::Focused | Self::OnFocusedWorkspace)
    }
}

pub struct AnchoredToast<'a> {
    pub id: u32,
    pub anchor: ToastAnchor,
    pub card: Element<'a, NotificationMessage>,
    pub motion: MotionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastWindow {
    Anchored(u64),
    Unanchored,
}

impl ToastWindow {
    pub fn resolve(data: &NotificationData, placement: &Placement) -> Self {
        let resolved = data
            .window_id_hint
            .or_else(|| placement.window_of_process_ancestry(&data.sender_ancestry))
            .or_else(|| {
                data.desktop_entry
                    .clone()
                    .and_then(|entry| placement.window_owned_by(&WindowOwner::App(entry)))
            });
        tracing::debug!("toast {}: window {resolved:?}", data.id);
        resolved.map_or(Self::Unanchored, Self::Anchored)
    }

    #[must_use]
    pub fn focus_within(self, placement: &Placement) -> AnchoredWindowFocus {
        match self.open_in(placement) {
            None => AnchoredWindowFocus::Unknown,
            Some(id) if placement.focused_window == Some(id) => AnchoredWindowFocus::Focused,
            Some(id) if placement.window_on_focused_workspace(id) => {
                AnchoredWindowFocus::OnFocusedWorkspace
            }
            Some(_) => AnchoredWindowFocus::Elsewhere,
        }
    }

    #[must_use]
    pub fn open_in(self, placement: &Placement) -> Option<u64> {
        match self {
            Self::Anchored(id) => placement.workspace_of_window(id).map(|_| id),
            Self::Unanchored => None,
        }
    }
}

#[must_use]
pub fn delegated_sender_window(data: &NotificationData, placement: &Placement) -> Option<u64> {
    data.window_id_hint.or_else(|| {
        let (&sender_pid, ancestors) = data.sender_ancestry.split_first()?;
        if placement.process_owns_window(sender_pid) {
            return None;
        }
        placement.window_of_process_ancestry(ancestors)
    })
}

#[must_use]
pub fn placement_for(
    window: ToastWindow,
    placement: &Placement,
    notifications: NotificationSettings,
    config: &Config,
) -> ToastPlacement {
    let id = match notifications.toast_arrangement {
        ToastArrangement::BesideSender => window.open_in(placement),
        ToastArrangement::ScreenCorner => None,
    };

    let window_focus = window.focus_within(placement);
    let Some(id) = id else {
        let fallback_corner = config.notification_toasts.placement.fallback_corner;
        return ToastPlacement {
            window,
            window_focus,
            output: fallback_output(placement),
            anchor: ToastAnchor::Edge {
                vertical: fallback_corner.vertical_edge(),
                horizontal: fallback_corner.horizontal_edge(),
            },
        };
    };

    let edge = placement.window_edge(id);
    let output = edge.output.or_else(|| fallback_output(placement));
    let anchor = match viewed_window_bounds(placement, id) {
        Some(bounds) => ToastAnchor::Window { id, bounds },
        None => ToastAnchor::Edge {
            vertical: edge.vertical,
            horizontal: edge.horizontal,
        },
    };
    ToastPlacement {
        window,
        window_focus,
        output,
        anchor,
    }
}

fn fallback_output(placement: &Placement) -> Option<String> {
    placement
        .focused_window
        .and_then(|w| placement.workspace_of_window(w))
        .and_then(|ws| placement.output_of_workspace(ws))
        .or(placement.focused_output.as_deref())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::{
        compositor_test_windows::{unnamed_compositor_workspace, unplaced_compositor_window},
        compositor_window_facts::{WindowFocus, WindowPid, WindowWorkspace, WorkspaceActivity},
        compositor_window_state::{CompositorWindow, CompositorWindowState},
    };
    use sottomano_desktop_sources::{
        desktop_notifications::notification_data::NotificationData,
        desktop_windows_apps::window_edge_placement::Placement,
    };

    use super::{AnchoredWindowFocus, ToastWindow, delegated_sender_window};

    const TERMINAL_WINDOW: u64 = 10;
    const TERMINAL_PID: i32 = 300;
    const BROWSER_WINDOW: u64 = 20;
    const BROWSER_PID: i32 = 900;

    fn desktop(focused_window: u64, terminal_workspace: u64) -> Placement {
        let mut away_workspace = unnamed_compositor_workspace(2);
        away_workspace.activity = WorkspaceActivity::Active;
        let window = |id, pid, workspace| CompositorWindow {
            pid: WindowPid::Reported(pid),
            workspace: WindowWorkspace::OnWorkspace(workspace),
            focus: if id == focused_window {
                WindowFocus::Focused
            } else {
                WindowFocus::Unfocused
            },
            ..unplaced_compositor_window(id)
        };
        Placement::derive(&CompositorWindowState::indexed_by_id(
            [unnamed_compositor_workspace(1), away_workspace],
            [
                window(TERMINAL_WINDOW, TERMINAL_PID, terminal_workspace),
                window(BROWSER_WINDOW, BROWSER_PID, 1),
            ],
        ))
    }

    fn sent_through(ancestry: Vec<u32>) -> NotificationData {
        NotificationData {
            sender_ancestry: ancestry,
            ..NotificationData::default()
        }
    }

    #[test]
    fn a_toast_resolves_to_the_terminal_hosting_its_notify_send() {
        let placement = desktop(BROWSER_WINDOW, 2);
        let resolved = ToastWindow::resolve(&sent_through(vec![500, 400, 300]), &placement);
        assert_eq!(resolved, ToastWindow::Anchored(TERMINAL_WINDOW));
    }

    #[test]
    fn a_helper_sender_delegates_to_its_window_owning_ancestor() {
        let placement = desktop(BROWSER_WINDOW, 2);
        assert_eq!(
            delegated_sender_window(&sent_through(vec![500, 400, 300]), &placement),
            Some(TERMINAL_WINDOW)
        );
    }

    #[test]
    fn an_app_notifying_from_its_own_window_does_not_delegate() {
        let placement = desktop(BROWSER_WINDOW, 2);
        assert_eq!(
            delegated_sender_window(&sent_through(vec![900]), &placement),
            None
        );
    }

    #[test]
    fn moving_a_window_is_pointless_when_it_shares_the_focused_workspace() {
        let terminal = ToastWindow::Anchored(TERMINAL_WINDOW);
        assert_eq!(
            terminal.focus_within(&desktop(TERMINAL_WINDOW, 1)),
            AnchoredWindowFocus::Focused
        );
        assert!(
            terminal
                .focus_within(&desktop(BROWSER_WINDOW, 1))
                .makes_move_pointless()
        );
        assert!(
            !terminal
                .focus_within(&desktop(BROWSER_WINDOW, 2))
                .makes_move_pointless()
        );
    }

    #[test]
    fn an_unanchored_toast_has_unknown_window_focus() {
        assert_eq!(
            ToastWindow::Unanchored.focus_within(&desktop(BROWSER_WINDOW, 1)),
            AnchoredWindowFocus::Unknown
        );
    }
}
