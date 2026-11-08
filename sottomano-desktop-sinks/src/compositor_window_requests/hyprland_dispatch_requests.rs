use sottomano_desktop_sources::compositor_window_ipc::hyprland_socket_connection::hyprland_request;

use crate::compositor_window_requests::{
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    systemd_run_launches::launch_in_user_service,
};

const HYPRLAND_ACCEPTED_REPLY: &str = "ok";

enum HyprlandDispatch {
    Dispatcher(String),
    Launch(Vec<String>),
    Unsupported,
}

fn window_address(window: u64) -> String {
    format!("address:0x{window:x}")
}

fn hyprland_dispatch_of(request: &CompositorRequest) -> HyprlandDispatch {
    let dispatcher = |dispatcher: String| HyprlandDispatch::Dispatcher(dispatcher);
    match request {
        CompositorRequest::FocusWindow(window) => {
            dispatcher(format!("focuswindow {}", window_address(*window)))
        }
        CompositorRequest::CloseWindow(window) => {
            dispatcher(format!("closewindow {}", window_address(*window)))
        }
        CompositorRequest::CloseFocusedWindow => dispatcher("killactive".to_owned()),
        CompositorRequest::FocusWorkspace(workspace) => {
            dispatcher(format!("workspace {workspace}"))
        }
        CompositorRequest::MoveWindowToWorkspace {
            window,
            workspace,
            follows_focus,
        } => {
            let mover = if *follows_focus {
                "movetoworkspace"
            } else {
                "movetoworkspacesilent"
            };
            dispatcher(format!("{mover} {workspace},{}", window_address(*window)))
        }
        CompositorRequest::FocusLeft => dispatcher("movefocus l".to_owned()),
        CompositorRequest::FocusRight => dispatcher("movefocus r".to_owned()),
        CompositorRequest::FocusUp => dispatcher("movefocus u".to_owned()),
        CompositorRequest::FocusDown => dispatcher("movefocus d".to_owned()),
        CompositorRequest::MoveLeft => dispatcher("movewindow l".to_owned()),
        CompositorRequest::MoveRight => dispatcher("movewindow r".to_owned()),
        CompositorRequest::FocusWorkspaceUp => dispatcher("workspace m-1".to_owned()),
        CompositorRequest::FocusWorkspaceDown => dispatcher("workspace m+1".to_owned()),
        CompositorRequest::MoveFocusedWindowToWorkspaceUp => {
            dispatcher("movetoworkspace m-1".to_owned())
        }
        CompositorRequest::MoveFocusedWindowToWorkspaceDown => {
            dispatcher("movetoworkspace m+1".to_owned())
        }
        CompositorRequest::FocusPreviousWindow => dispatcher("focuscurrentorlast".to_owned()),
        CompositorRequest::MaximizeFocused => dispatcher("fullscreen 1".to_owned()),
        CompositorRequest::CenterFocused => dispatcher("centerwindow".to_owned()),
        CompositorRequest::PowerOffMonitors => dispatcher("dpms off".to_owned()),
        CompositorRequest::Quit => dispatcher("exit".to_owned()),
        CompositorRequest::Spawn(command) => HyprlandDispatch::Launch(command.clone()),
        CompositorRequest::MoveWorkspaceToIndex { .. }
        | CompositorRequest::MoveNamedWorkspaceToIndex { .. }
        | CompositorRequest::MoveFocusedColumnToIndex(_)
        | CompositorRequest::ToggleOverview => HyprlandDispatch::Unsupported,
    }
}

pub fn send_hyprland_request(request: &CompositorRequest) -> Result<(), CompositorRequestFailure> {
    match hyprland_dispatch_of(request) {
        HyprlandDispatch::Launch(command) => launch_in_user_service(&command),
        HyprlandDispatch::Unsupported => Err(CompositorRequestFailure::Unsupported {
            request: format!("{request:?}"),
            compositor: "Hyprland",
        }),
        HyprlandDispatch::Dispatcher(dispatcher) => {
            let reply =
                hyprland_request(&format!("dispatch {dispatcher}")).map_err(|request_error| {
                    CompositorRequestFailure::Unreachable {
                        request: dispatcher.clone(),
                        reason: request_error.to_string(),
                    }
                })?;
            if reply.trim() == HYPRLAND_ACCEPTED_REPLY {
                Ok(())
            } else {
                Err(CompositorRequestFailure::Rejected {
                    request: dispatcher,
                    reason: reply.trim().to_owned(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HyprlandDispatch, hyprland_dispatch_of};
    use crate::compositor_window_requests::compositor_requests::CompositorRequest;

    fn dispatcher(request: &CompositorRequest) -> Option<String> {
        match hyprland_dispatch_of(request) {
            HyprlandDispatch::Dispatcher(dispatcher) => Some(dispatcher),
            HyprlandDispatch::Launch(_) | HyprlandDispatch::Unsupported => None,
        }
    }

    #[test]
    fn windows_are_addressed_by_their_hex_address() {
        assert_eq!(
            dispatcher(&CompositorRequest::FocusWindow(0x5a1)).as_deref(),
            Some("focuswindow address:0x5a1")
        );
    }

    #[test]
    fn an_unfollowed_move_stays_silent() {
        assert_eq!(
            dispatcher(&CompositorRequest::MoveWindowToWorkspace {
                window: 0x5a1,
                workspace: 3,
                follows_focus: false,
            })
            .as_deref(),
            Some("movetoworkspacesilent 3,address:0x5a1")
        );
    }
}
