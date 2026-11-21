use sottomano_compositor_model::compositor_requests::{
    CompositorRequest, CompositorRequestFailure,
};

use crate::{
    hyprland_lua_rules::lua_string, hyprland_socket_connection::hyprland_request,
    hyprland_workspace_identity::RegularHyprlandWorkspace,
};

const HYPRLAND_ACCEPTED_REPLY: &str = "ok";

const HYPREXPO_OVERVIEW_TOGGLE: &str = "hl.plugin.hyprexpo.expo(\"toggle\")";

pub(crate) enum HyprlandDispatch {
    Dispatcher(String),
    Evaluation(&'static str),
    Command(&'static str),
    Launch(Vec<String>),
    WorkspaceGone,
    Unsupported,
}

fn window_selector(window: u64) -> String {
    format!("\"address:0x{window:x}\"")
}

fn workspace_selector(workspace: &RegularHyprlandWorkspace) -> String {
    match workspace {
        RegularHyprlandWorkspace::Numbered { id } => id.to_string(),
        RegularHyprlandWorkspace::Addressed { address } => lua_string(&format!("name:{address}")),
    }
}

fn lua_dispatcher(expression: &str) -> HyprlandDispatch {
    HyprlandDispatch::Dispatcher(format!("hl.dsp.{expression}"))
}

impl HyprlandDispatch {
    pub(crate) fn new(
        request: &CompositorRequest,
        mut listed_workspace: impl FnMut(u64) -> Option<RegularHyprlandWorkspace>,
    ) -> Self {
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
            CompositorRequest::FocusWorkspace(workspace_key) => {
                let Some(workspace) = listed_workspace(*workspace_key) else {
                    return HyprlandDispatch::WorkspaceGone;
                };
                lua_dispatcher(&format!(
                    "focus({{ workspace = {} }})",
                    workspace_selector(&workspace)
                ))
            }
            CompositorRequest::MoveWindowToWorkspace {
                window,
                workspace: workspace_key,
                follows_focus,
            } => {
                let Some(workspace) = listed_workspace(*workspace_key) else {
                    return HyprlandDispatch::WorkspaceGone;
                };
                lua_dispatcher(&format!(
                    "window.move({{ workspace = {}, follow = {follows_focus}, window = {} }})",
                    workspace_selector(&workspace),
                    window_selector(*window)
                ))
            }
            CompositorRequest::FocusLeft => lua_dispatcher("focus({ direction = \"left\" })"),
            CompositorRequest::FocusRight => lua_dispatcher("focus({ direction = \"right\" })"),
            CompositorRequest::FocusUp => lua_dispatcher("focus({ direction = \"up\" })"),
            CompositorRequest::FocusDown => lua_dispatcher("focus({ direction = \"down\" })"),
            CompositorRequest::MoveLeft => lua_dispatcher("window.move({ direction = \"left\" })"),
            CompositorRequest::MoveRight => {
                lua_dispatcher("window.move({ direction = \"right\" })")
            }
            CompositorRequest::FocusWorkspaceUp => lua_dispatcher("focus({ workspace = \"m-1\" })"),
            CompositorRequest::FocusWorkspaceDown => {
                lua_dispatcher("focus({ workspace = \"m+1\" })")
            }
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
            CompositorRequest::ToggleOverview => {
                HyprlandDispatch::Evaluation(HYPREXPO_OVERVIEW_TOGGLE)
            }
            CompositorRequest::MoveWorkspaceToIndex { .. }
            | CompositorRequest::MoveNamedWorkspaceToIndex { .. }
            | CompositorRequest::MoveFocusedColumnToIndex(_) => HyprlandDispatch::Unsupported,
        }
    }
}

pub(crate) async fn hyprland_command_accepted(
    command: String,
) -> Result<(), CompositorRequestFailure> {
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

pub(crate) fn referenced_workspace_key(request: &CompositorRequest) -> Option<u64> {
    match request {
        CompositorRequest::FocusWorkspace(workspace_key)
        | CompositorRequest::MoveWindowToWorkspace {
            workspace: workspace_key,
            ..
        } => Some(*workspace_key),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::compositor_requests::CompositorRequest;

    use super::HyprlandDispatch;
    use crate::hyprland_workspace_identity::RegularHyprlandWorkspace;

    fn dispatcher(request: &CompositorRequest) -> Option<String> {
        match HyprlandDispatch::new(request, |id| {
            Some(RegularHyprlandWorkspace::Numbered { id })
        }) {
            HyprlandDispatch::Dispatcher(dispatcher) => Some(dispatcher),
            HyprlandDispatch::Launch(_)
            | HyprlandDispatch::Evaluation(_)
            | HyprlandDispatch::Command(_)
            | HyprlandDispatch::WorkspaceGone
            | HyprlandDispatch::Unsupported => None,
        }
    }

    #[test]
    fn the_overview_toggles_through_the_hyprexpo_plugin() {
        assert!(matches!(
            HyprlandDispatch::new(&CompositorRequest::ToggleOverview, |_| None),
            HyprlandDispatch::Evaluation("hl.plugin.hyprexpo.expo(\"toggle\")")
        ));
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

    #[test]
    fn workspaces_without_a_number_are_focused_by_their_quoted_address() {
        let focus_games = HyprlandDispatch::new(&CompositorRequest::FocusWorkspace(7), |_| {
            Some(RegularHyprlandWorkspace::Addressed {
                address: "Gam\"es".to_owned(),
            })
        });

        assert!(matches!(
            focus_games,
            HyprlandDispatch::Dispatcher(dispatcher)
                if dispatcher == "hl.dsp.focus({ workspace = \"name:Gam\\\"es\" })"
        ));
    }

    #[test]
    fn a_workspace_hyprland_no_longer_lists_is_reported_gone() {
        assert!(matches!(
            HyprlandDispatch::new(&CompositorRequest::FocusWorkspace(7), |_| None),
            HyprlandDispatch::WorkspaceGone
        ));
    }
}
