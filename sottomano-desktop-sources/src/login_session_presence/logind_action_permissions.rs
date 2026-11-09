use std::env;

use futures::future::join_all;

use super::{logind_manager::ManagerProxy, session_power_actions::SessionAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowed {
    pub action: SessionAction,
    pub needs_authorisation: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Permissions {
    pub allowed: Vec<Allowed>,
}

enum Answer {
    Granted,
    AfterChallenge,
    Denied,
}

impl Answer {
    fn always() -> Self {
        Answer::Granted
    }
}

impl From<&zbus::Result<String>> for Answer {
    fn from(reply: &zbus::Result<String>) -> Self {
        match reply.as_deref() {
            Ok("yes") => Answer::Granted,
            Ok("challenge") => Answer::AfterChallenge,
            Ok(_) => Answer::Denied,
            Err(e) => {
                tracing::debug!("logind capability query: {e}");
                Answer::Denied
            }
        }
    }
}

async fn answer_for(proxy: &ManagerProxy<'static>, action: SessionAction) -> Answer {
    match action {
        SessionAction::Lock
        | SessionAction::LockAllSessions
        | SessionAction::ScreenOff
        | SessionAction::LogOut
        | SessionAction::TerminateSession => Answer::always(),
        SessionAction::Sleep => Answer::from(&proxy.can_sleep().await),
        SessionAction::Suspend => Answer::from(&proxy.can_suspend().await),
        SessionAction::Hibernate => Answer::from(&proxy.can_hibernate().await),
        SessionAction::HybridSleep => Answer::from(&proxy.can_hybrid_sleep().await),
        SessionAction::SuspendThenHibernate => {
            Answer::from(&proxy.can_suspend_then_hibernate().await)
        }
        SessionAction::Reboot => Answer::from(&proxy.can_reboot().await),
        SessionAction::RebootToFirmwareSetup => {
            Answer::from(&proxy.can_reboot_to_firmware_setup().await)
        }
        SessionAction::RebootToBootLoaderMenu => {
            Answer::from(&proxy.can_reboot_to_boot_loader_menu().await)
        }
        SessionAction::Halt => Answer::from(&proxy.can_halt().await),
        SessionAction::PowerOff => Answer::from(&proxy.can_power_off().await),
        SessionAction::CancelScheduledShutdown => match proxy.scheduled_shutdown().await {
            Ok((kind, _)) if !kind.is_empty() => Answer::Granted,
            Ok(_) | Err(_) => Answer::Denied,
        },
        SessionAction::RestartShell => match env::var_os("INVOCATION_ID") {
            Some(_) => Answer::Granted,
            None => Answer::Denied,
        },
    }
}

async fn allowed(proxy: &ManagerProxy<'static>, action: SessionAction) -> Option<Allowed> {
    match answer_for(proxy, action).await {
        Answer::Granted => Some(Allowed {
            action,
            needs_authorisation: false,
        }),
        Answer::AfterChallenge => Some(Allowed {
            action,
            needs_authorisation: true,
        }),
        Answer::Denied => None,
    }
}

pub async fn read() -> Permissions {
    let Ok(system) = zbus::Connection::system().await else {
        return Permissions::default();
    };
    let Ok(proxy) = ManagerProxy::new(&system).await else {
        return Permissions::default();
    };
    Permissions {
        allowed: join_all(
            SessionAction::ALL
                .into_iter()
                .map(|action| allowed(&proxy, action)),
        )
        .await
        .into_iter()
        .flatten()
        .collect(),
    }
}
