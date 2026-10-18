use std::env;

use crate::data::source::{
    logind::{ManagerProxyBlocking, manager},
    session_action::SessionAction,
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
    fn of_logind(reply: &zbus::Result<String>) -> Self {
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

    fn always() -> Self {
        Answer::Granted
    }

    fn when(condition: bool) -> Self {
        if condition {
            Answer::Granted
        } else {
            Answer::Denied
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
        SessionAction::Sleep => Answer::of_logind(&proxy.can_sleep()),
        SessionAction::Suspend => Answer::of_logind(&proxy.can_suspend()),
        SessionAction::Hibernate => Answer::of_logind(&proxy.can_hibernate()),
        SessionAction::HybridSleep => Answer::of_logind(&proxy.can_hybrid_sleep()),
        SessionAction::SuspendThenHibernate => {
            Answer::of_logind(&proxy.can_suspend_then_hibernate())
        }
        SessionAction::Reboot => Answer::of_logind(&proxy.can_reboot()),
        SessionAction::RebootToFirmwareSetup => {
            Answer::of_logind(&proxy.can_reboot_to_firmware_setup())
        }
        SessionAction::RebootToBootLoaderMenu => {
            Answer::of_logind(&proxy.can_reboot_to_boot_loader_menu())
        }
        SessionAction::Halt => Answer::of_logind(&proxy.can_halt()),
        SessionAction::PowerOff => Answer::of_logind(&proxy.can_power_off()),
        SessionAction::CancelScheduledShutdown => Answer::when(
            proxy
                .scheduled_shutdown()
                .is_ok_and(|(kind, _)| !kind.is_empty()),
        ),
        SessionAction::RestartShell => Answer::when(env::var_os("INVOCATION_ID").is_some()),
    }
}

#[must_use]
pub fn read() -> Permissions {
    let Some(proxy) = manager() else {
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
