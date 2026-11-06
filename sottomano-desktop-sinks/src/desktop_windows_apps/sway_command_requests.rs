use serde::Deserialize;
use sottomano_desktop_sources::desktop_windows_apps::sway_ipc_connection::{
    SwayIpcConnection, SwayMessageKind,
};

use crate::desktop_windows_apps::{
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    systemd_run_launches::launch_in_user_service,
};

#[derive(Deserialize)]
struct SwayCommandOutcome {
    success: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SwayWorkspaceName {
    id: u64,
    name: String,
}

enum SwayCommand {
    Text(String),
    Launch(Vec<String>),
    Unsupported,
}

fn quoted_workspace(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

fn sway_command_of(
    request: &CompositorRequest,
    mut workspace_name: impl FnMut(u64) -> Option<String>,
) -> SwayCommand {
    let text = |command: String| SwayCommand::Text(command);
    match request {
        CompositorRequest::FocusWindow(window) => text(format!("[con_id={window}] focus")),
        CompositorRequest::CloseWindow(window) => text(format!("[con_id={window}] kill")),
        CompositorRequest::CloseFocusedWindow => text("kill".to_owned()),
        CompositorRequest::FocusWorkspace(workspace) => workspace_name(*workspace)
            .map_or(SwayCommand::Unsupported, |name| {
                text(format!("workspace {}", quoted_workspace(&name)))
            }),
        CompositorRequest::MoveWindowToWorkspace {
            window,
            workspace,
            follows_focus,
        } => workspace_name(*workspace).map_or(SwayCommand::Unsupported, |name| {
            let target = quoted_workspace(&name);
            let moved = format!("[con_id={window}] move container to workspace {target}");
            text(if *follows_focus {
                format!("{moved}; workspace {target}; [con_id={window}] focus")
            } else {
                moved
            })
        }),
        CompositorRequest::FocusLeft => text("focus left".to_owned()),
        CompositorRequest::FocusRight => text("focus right".to_owned()),
        CompositorRequest::FocusUp => text("focus up".to_owned()),
        CompositorRequest::FocusDown => text("focus down".to_owned()),
        CompositorRequest::MoveLeft => text("move left".to_owned()),
        CompositorRequest::MoveRight => text("move right".to_owned()),
        CompositorRequest::FocusWorkspaceUp => text("workspace prev_on_output".to_owned()),
        CompositorRequest::FocusWorkspaceDown => text("workspace next_on_output".to_owned()),
        CompositorRequest::MoveFocusedWindowToWorkspaceUp => {
            text("move container to workspace prev_on_output; workspace prev_on_output".to_owned())
        }
        CompositorRequest::MoveFocusedWindowToWorkspaceDown => {
            text("move container to workspace next_on_output; workspace next_on_output".to_owned())
        }
        CompositorRequest::PowerOffMonitors => text("output * power off".to_owned()),
        CompositorRequest::Quit => text("exit".to_owned()),
        CompositorRequest::Spawn(command) => SwayCommand::Launch(command.clone()),
        CompositorRequest::MoveWorkspaceToIndex { .. }
        | CompositorRequest::MoveNamedWorkspaceToIndex { .. }
        | CompositorRequest::MoveFocusedColumnToIndex(_)
        | CompositorRequest::FocusPreviousWindow
        | CompositorRequest::ToggleOverview
        | CompositorRequest::MaximizeFocused
        | CompositorRequest::CenterFocused => SwayCommand::Unsupported,
    }
}

fn sway_workspace_name(connection: &mut SwayIpcConnection, workspace: u64) -> Option<String> {
    let reply = connection
        .request(SwayMessageKind::GetWorkspaces, "")
        .ok()?;
    serde_json::from_str::<Vec<SwayWorkspaceName>>(&reply)
        .ok()?
        .into_iter()
        .find(|listed| listed.id == workspace)
        .map(|listed| listed.name)
}

pub fn send_sway_request(request: &CompositorRequest) -> Result<(), CompositorRequestFailure> {
    let unreachable = |reason: String| CompositorRequestFailure::Unreachable {
        request: format!("{request:?}"),
        reason,
    };
    let mut connection = SwayIpcConnection::connect()
        .map_err(|connect_error| unreachable(connect_error.to_string()))?;
    let command = sway_command_of(request, |workspace| {
        sway_workspace_name(&mut connection, workspace)
    });
    match command {
        SwayCommand::Launch(command) => launch_in_user_service(&command),
        SwayCommand::Unsupported => Err(CompositorRequestFailure::Unsupported {
            request: format!("{request:?}"),
            compositor: "sway",
        }),
        SwayCommand::Text(command) => {
            let reply = connection
                .request(SwayMessageKind::RunCommand, &command)
                .map_err(|request_error| unreachable(request_error.to_string()))?;
            let outcomes: Vec<SwayCommandOutcome> = serde_json::from_str(&reply)
                .map_err(|parse_error| unreachable(parse_error.to_string()))?;
            match outcomes.into_iter().find(|outcome| !outcome.success) {
                None => Ok(()),
                Some(refused) => Err(CompositorRequestFailure::Rejected {
                    request: command,
                    reason: refused.error.unwrap_or_default(),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SwayCommand, sway_command_of};
    use crate::desktop_windows_apps::compositor_requests::CompositorRequest;

    fn command_text(request: &CompositorRequest) -> Option<String> {
        match sway_command_of(request, |_| Some("2: \"mail\"".to_owned())) {
            SwayCommand::Text(command) => Some(command),
            SwayCommand::Launch(_) | SwayCommand::Unsupported => None,
        }
    }

    #[test]
    fn a_followed_move_switches_to_the_quoted_workspace_and_refocuses_the_window() {
        assert_eq!(
            command_text(&CompositorRequest::MoveWindowToWorkspace {
                window: 7,
                workspace: 2,
                follows_focus: true,
            })
            .as_deref(),
            Some(
                "[con_id=7] move container to workspace \"2: \\\"mail\\\"\"; workspace \"2: \
                 \\\"mail\\\"\"; [con_id=7] focus"
            )
        );
    }

    #[test]
    fn requests_without_a_sway_equivalent_are_unsupported() {
        assert!(matches!(
            sway_command_of(&CompositorRequest::ToggleOverview, |_| None),
            SwayCommand::Unsupported
        ));
        assert!(matches!(
            sway_command_of(&CompositorRequest::FocusWorkspace(9), |_| None),
            SwayCommand::Unsupported
        ));
    }
}
