use std::env;

use super::{
    logind_manager::{ManagerProxyBlocking, logind_manager_proxy},
    session_power_actions::SessionAction,
};

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

fn answer_for(proxy: &ManagerProxyBlocking<'static>, action: SessionAction) -> Answer {
    match action {
        SessionAction::Lock
        | SessionAction::LockAllSessions
        | SessionAction::ScreenOff
        | SessionAction::LogOut
        | SessionAction::TerminateSession => Answer::always(),
        SessionAction::Sleep => Answer::from(&proxy.can_sleep()),
        SessionAction::Suspend => Answer::from(&proxy.can_suspend()),
        SessionAction::Hibernate => Answer::from(&proxy.can_hibernate()),
        SessionAction::HybridSleep => Answer::from(&proxy.can_hybrid_sleep()),
        SessionAction::SuspendThenHibernate => Answer::from(&proxy.can_suspend_then_hibernate()),
        SessionAction::Reboot => Answer::from(&proxy.can_reboot()),
        SessionAction::RebootToFirmwareSetup => Answer::from(&proxy.can_reboot_to_firmware_setup()),
        SessionAction::RebootToBootLoaderMenu => {
            Answer::from(&proxy.can_reboot_to_boot_loader_menu())
        }
        SessionAction::Halt => Answer::from(&proxy.can_halt()),
        SessionAction::PowerOff => Answer::from(&proxy.can_power_off()),
        SessionAction::CancelScheduledShutdown => match proxy.scheduled_shutdown() {
            Ok((kind, _)) if !kind.is_empty() => Answer::Granted,
            Ok(_) | Err(_) => Answer::Denied,
        },
        SessionAction::RestartShell => match env::var_os("INVOCATION_ID") {
            Some(_) => Answer::Granted,
            None => Answer::Denied,
        },
    }
}

#[must_use]
pub fn read() -> Permissions {
    let Some(proxy) = logind_manager_proxy() else {
        return Permissions::default();
    };
    Permissions {
        allowed: SessionAction::ALL
            .into_iter()
            .filter_map(|action| match answer_for(&proxy, action) {
                Answer::Granted => Some(Allowed {
                    action,
                    needs_authorisation: false,
                }),
                Answer::AfterChallenge => Some(Allowed {
                    action,
                    needs_authorisation: true,
                }),
                Answer::Denied => None,
            })
            .collect(),
    }
}
