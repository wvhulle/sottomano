use sottomano_desktop_sources::compositor_window_ipc::hyprland_socket_connection::hyprland_request;

use crate::compositor_window_requests::{
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    systemd_run_launches::launch_in_user_service,
};

const HYPRLAND_ACCEPTED_REPLY: &str = "ok";

enum HyprlandDispatch {
    Dispatcher(String),
    Command(&'static str),
    Launch(Vec<String>),
    Unsupported,
}

fn window_selector(window: u64) -> String {
    format!("\"address:0x{window:x}\"")
}

fn lua_dispatcher(expression: &str) -> HyprlandDispatch {
    HyprlandDispatch::Dispatcher(format!("hl.dsp.{expression}"))
}

fn hyprland_dispatch_of(request: &CompositorRequest) -> HyprlandDispatch {
    match request {
        CompositorRequest::FocusWindow(window) => lua_dispatcher(&format!(
            "focus({{ window = {} }})",
            window_selector(*window)
        )),
        CompositorRequest::CloseWindow(window) => lua_dispatcher(&format!(
            "window.close({{ window = {} }})",
            window_selector(*window)
        )),
        CompositorRequest::CloseFocusedWindow => lua_dispatcher("window.close()"),
        CompositorRequest::FocusWorkspace(workspace) => {
            lua_dispatcher(&format!("focus({{ workspace = {workspace} }})"))
        }
        CompositorRequest::MoveWindowToWorkspace {
            window,
            workspace,
            follows_focus,
        } => lua_dispatcher(&format!(
            "window.move({{ workspace = {workspace}, follow = {follows_focus}, window = {} }})",
            window_selector(*window)
        )),
        CompositorRequest::FocusLeft => lua_dispatcher("focus({ direction = \"left\" })"),
        CompositorRequest::FocusRight => lua_dispatcher("focus({ direction = \"right\" })"),
        CompositorRequest::FocusUp => lua_dispatcher("focus({ direction = \"up\" })"),
        CompositorRequest::FocusDown => lua_dispatcher("focus({ direction = \"down\" })"),
        CompositorRequest::MoveLeft => lua_dispatcher("window.move({ direction = \"left\" })"),
        CompositorRequest::MoveRight => lua_dispatcher("window.move({ direction = \"right\" })"),
        CompositorRequest::FocusWorkspaceUp => lua_dispatcher("focus({ workspace = \"m-1\" })"),
        CompositorRequest::FocusWorkspaceDown => lua_dispatcher("focus({ workspace = \"m+1\" })"),
        CompositorRequest::MoveFocusedWindowToWorkspaceUp => {
            lua_dispatcher("window.move({ workspace = \"m-1\" })")
        }
        CompositorRequest::MoveFocusedWindowToWorkspaceDown => {
            lua_dispatcher("window.move({ workspace = \"m+1\" })")
        }
        CompositorRequest::FocusPreviousWindow => lua_dispatcher("focus({ last = true })"),
        CompositorRequest::MaximizeFocused => {
            lua_dispatcher("window.fullscreen({ mode = \"maximized\" })")
        }
        CompositorRequest::CenterFocused => lua_dispatcher("window.center()"),
        CompositorRequest::PowerOffMonitors => lua_dispatcher("dpms({ action = \"off\" })"),
        CompositorRequest::Quit => lua_dispatcher("exit()"),
        CompositorRequest::ReloadConfig => HyprlandDispatch::Command("reload"),
        CompositorRequest::Spawn(command) => HyprlandDispatch::Launch(command.clone()),
        CompositorRequest::MoveWorkspaceToIndex { .. }
        | CompositorRequest::MoveNamedWorkspaceToIndex { .. }
        | CompositorRequest::MoveFocusedColumnToIndex(_)
        | CompositorRequest::ToggleOverview => HyprlandDispatch::Unsupported,
    }
}

async fn hyprland_command_accepted(command: String) -> Result<(), CompositorRequestFailure> {
    let reply = hyprland_request(&command).await.map_err(|request_error| {
        CompositorRequestFailure::Unreachable {
            request: command.clone(),
            reason: request_error.to_string(),
        }
    })?;
    if reply.trim() == HYPRLAND_ACCEPTED_REPLY {
        Ok(())
    } else {
        Err(CompositorRequestFailure::Rejected {
            request: command,
            reason: reply.trim().to_owned(),
        })
    }
}

pub async fn send_hyprland_request(
    request: &CompositorRequest,
) -> Result<(), CompositorRequestFailure> {
    match hyprland_dispatch_of(request) {
        HyprlandDispatch::Launch(command) => launch_in_user_service(&command).await,
        HyprlandDispatch::Unsupported => Err(CompositorRequestFailure::Unsupported {
            request: format!("{request:?}"),
            compositor: "Hyprland",
        }),
        HyprlandDispatch::Dispatcher(dispatcher) => {
            hyprland_command_accepted(format!("dispatch {dispatcher}")).await
        }
        HyprlandDispatch::Command(command) => hyprland_command_accepted(command.to_owned()).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{HyprlandDispatch, hyprland_dispatch_of};
    use crate::compositor_window_requests::compositor_requests::CompositorRequest;

    fn dispatcher(request: &CompositorRequest) -> Option<String> {
        match hyprland_dispatch_of(request) {
            HyprlandDispatch::Dispatcher(dispatcher) => Some(dispatcher),
            HyprlandDispatch::Launch(_)
            | HyprlandDispatch::Command(_)
            | HyprlandDispatch::Unsupported => None,
        }
    }

    #[test]
    fn windows_are_addressed_by_their_hex_address() {
        assert_eq!(
            dispatcher(&CompositorRequest::FocusWindow(0x5a1)).as_deref(),
            Some("hl.dsp.focus({ window = \"address:0x5a1\" })")
        );
    }

    #[test]
    fn an_unfollowed_move_does_not_follow_focus() {
        assert_eq!(
            dispatcher(&CompositorRequest::MoveWindowToWorkspace {
                window: 0x5a1,
                workspace: 3,
                follows_focus: false,
            })
            .as_deref(),
            Some(
                "hl.dsp.window.move({ workspace = 3, follow = false, window = \"address:0x5a1\" })"
            )
        );
    }
}
