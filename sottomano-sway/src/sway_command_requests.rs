use sottomano_compositor_model::compositor_requests::{CompositorRequest, FocusFollow};
use swayipc_types::{CommandType, Fallible, Workspace};

use crate::sway_ipc_connection::SwayIpcConnection;

pub(crate) enum SwayCommand {
    Text(String),
    Launch(Vec<String>),
    Unsupported,
}

fn quoted_workspace(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

impl SwayCommand {
    pub(crate) fn new(
        request: &CompositorRequest,
        mut workspace_name: impl FnMut(u64) -> Option<String>,
    ) -> Self {
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
                focus_follow,
            } => workspace_name(*workspace).map_or(SwayCommand::Unsupported, |name| {
                let target = quoted_workspace(&name);
                let moved = format!("[con_id={window}] move container to workspace {target}");
                text(match focus_follow {
                    FocusFollow::FollowWindow => {
                        format!("{moved}; workspace {target}; [con_id={window}] focus")
                    }
                    FocusFollow::StayOnCurrent => moved,
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
            CompositorRequest::MoveFocusedWindowToWorkspaceUp => text(
                "move container to workspace prev_on_output; workspace prev_on_output".to_owned(),
            ),
            CompositorRequest::MoveFocusedWindowToWorkspaceDown => text(
                "move container to workspace next_on_output; workspace next_on_output".to_owned(),
            ),
            CompositorRequest::PowerOffMonitors => text("output * power off".to_owned()),
            CompositorRequest::Quit => text("exit".to_owned()),
            CompositorRequest::ReloadConfig => text("reload".to_owned()),
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
}

pub(crate) fn referenced_workspace(request: &CompositorRequest) -> Option<u64> {
    match request {
        CompositorRequest::FocusWorkspace(workspace)
        | CompositorRequest::MoveWindowToWorkspace { workspace, .. } => Some(*workspace),
        _ => None,
    }
}

pub(crate) async fn sway_workspace_name(
    connection: &mut SwayIpcConnection,
    workspace: u64,
) -> Fallible<Option<String>> {
    let workspaces: Vec<Workspace> = connection.request(CommandType::GetWorkspaces, "").await?;
    Ok(workspaces
        .into_iter()
        .find(|listed| u64::try_from(listed.id) == Ok(workspace))
        .map(|listed| listed.name))
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::compositor_requests::{CompositorRequest, FocusFollow};

    use super::SwayCommand;

    fn command_text(request: &CompositorRequest) -> Option<String> {
        match SwayCommand::new(request, |_| Some("2: \"mail\"".to_owned())) {
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
                focus_follow: FocusFollow::FollowWindow,
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
            SwayCommand::new(&CompositorRequest::ToggleOverview, |_| None),
            SwayCommand::Unsupported
        ));
        assert!(matches!(
            SwayCommand::new(&CompositorRequest::FocusWorkspace(9), |_| None),
            SwayCommand::Unsupported
        ));
    }
}
