use std::fmt;

use niri_ipc::{Request, socket::Socket};

#[derive(Debug, Clone)]
pub enum ActionFailure {
    Rejected { action: String, reason: String },
    Unreachable { action: String, reason: String },
}

impl ActionFailure {
    #[must_use]
    pub fn summary(&self) -> &'static str {
        match self {
            ActionFailure::Rejected { .. } => "niri refused a request from the bar",
            ActionFailure::Unreachable { .. } => "The bar could not reach niri",
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            ActionFailure::Rejected { reason, .. } | ActionFailure::Unreachable { reason, .. } => {
                reason
            }
        }
    }

    #[must_use]
    pub fn action(&self) -> &str {
        match self {
            ActionFailure::Rejected { action, .. } | ActionFailure::Unreachable { action, .. } => {
                action
            }
        }
    }
}

impl fmt::Display for ActionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} ({})",
            self.summary(),
            self.reason(),
            self.action()
        )
    }
}

pub fn send_action(action: &niri_ipc::Action) -> Result<(), ActionFailure> {
    let sent =
        Socket::connect().and_then(|mut socket| socket.send(Request::Action(action.clone())));
    match sent {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(rejection)) => Err(ActionFailure::Rejected {
            action: format!("{action:?}"),
            reason: rejection,
        }),
        Err(e) => Err(ActionFailure::Unreachable {
            action: format!("{action:?}"),
            reason: e.to_string(),
        }),
    }
}

pub fn send_action_or_warn(action: &niri_ipc::Action) {
    if let Err(failure) = send_action(action) {
        tracing::warn!("{failure}");
    }
}
