use niri_ipc::{Action, Request, WorkspaceReferenceArg, socket::Socket};

use crate::compositor_window_requests::compositor_requests::{
    CompositorRequest, CompositorRequestFailure,
};

fn niri_action_of(request: &CompositorRequest) -> Action {
    match request {
        CompositorRequest::FocusWindow(window) => Action::FocusWindow { id: *window },
        CompositorRequest::CloseWindow(window) => Action::CloseWindow { id: Some(*window) },
        CompositorRequest::CloseFocusedWindow => Action::CloseWindow { id: None },
        CompositorRequest::FocusWorkspace(workspace) => Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Id(*workspace),
        },
        CompositorRequest::MoveWorkspaceToIndex { workspace, index } => {
            Action::MoveWorkspaceToIndex {
                index: *index,
                reference: Some(WorkspaceReferenceArg::Id(*workspace)),
            }
        }
        CompositorRequest::MoveNamedWorkspaceToIndex { name, index } => {
            Action::MoveWorkspaceToIndex {
                index: *index,
                reference: Some(WorkspaceReferenceArg::Name(name.clone())),
            }
        }
        CompositorRequest::MoveWindowToWorkspace {
            window,
            workspace,
            follows_focus,
        } => Action::MoveWindowToWorkspace {
            window_id: Some(*window),
            reference: WorkspaceReferenceArg::Id(*workspace),
            focus: *follows_focus,
        },
        CompositorRequest::MoveFocusedColumnToIndex(index) => {
            Action::MoveColumnToIndex { index: *index }
        }
        CompositorRequest::FocusLeft => Action::FocusColumnLeft {},
        CompositorRequest::FocusRight => Action::FocusColumnRight {},
        CompositorRequest::FocusUp => Action::FocusWindowUp {},
        CompositorRequest::FocusDown => Action::FocusWindowDown {},
        CompositorRequest::MoveLeft => Action::MoveColumnLeft {},
        CompositorRequest::MoveRight => Action::MoveColumnRight {},
        CompositorRequest::FocusWorkspaceUp => Action::FocusWorkspaceUp {},
        CompositorRequest::FocusWorkspaceDown => Action::FocusWorkspaceDown {},
        CompositorRequest::MoveFocusedWindowToWorkspaceUp => {
            Action::MoveWindowToWorkspaceUp { focus: true }
        }
        CompositorRequest::MoveFocusedWindowToWorkspaceDown => {
            Action::MoveWindowToWorkspaceDown { focus: true }
        }
        CompositorRequest::FocusPreviousWindow => Action::FocusWindowPrevious {},
        CompositorRequest::ToggleOverview => Action::ToggleOverview {},
        CompositorRequest::MaximizeFocused => Action::MaximizeColumn {},
        CompositorRequest::CenterFocused => Action::CenterColumn {},
        CompositorRequest::Spawn(command) => Action::Spawn {
            command: command.clone(),
        },
        CompositorRequest::PowerOffMonitors => Action::PowerOffMonitors {},
        CompositorRequest::Quit => Action::Quit {
            skip_confirmation: true,
        },
    }
}

pub fn send_niri_request(request: &CompositorRequest) -> Result<(), CompositorRequestFailure> {
    let action = niri_action_of(request);
    let sent = Socket::connect().and_then(|mut socket| socket.send(Request::Action(action)));
    match sent {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(rejection)) => Err(CompositorRequestFailure::Rejected {
            request: format!("{request:?}"),
            reason: rejection,
        }),
        Err(connection_error) => Err(CompositorRequestFailure::Unreachable {
            request: format!("{request:?}"),
            reason: connection_error.to_string(),
        }),
    }
}
