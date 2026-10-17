use crate::{
    data::source::{command, logind, niri},
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionAction {
    Lock,
    LockAllSessions,
    ScreenOff,
    Sleep,
    Suspend,
    Hibernate,
    HybridSleep,
    SuspendThenHibernate,
    Reboot,
    RebootToFirmwareSetup,
    RebootToBootLoaderMenu,
    Halt,
    PowerOff,
    CancelScheduledShutdown,
    LogOut,
    TerminateSession,
    RestartShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionFamily {
    Locking,
    Sleeping,
    Shutdown,
    Session,
}

impl SessionAction {
    pub const ALL: [SessionAction; 17] = [
        Self::Lock,
        Self::LockAllSessions,
        Self::ScreenOff,
        Self::Sleep,
        Self::Suspend,
        Self::Hibernate,
        Self::HybridSleep,
        Self::SuspendThenHibernate,
        Self::Reboot,
        Self::RebootToFirmwareSetup,
        Self::RebootToBootLoaderMenu,
        Self::Halt,
        Self::PowerOff,
        Self::CancelScheduledShutdown,
        Self::LogOut,
        Self::TerminateSession,
        Self::RestartShell,
    ];

    pub fn family(self) -> ActionFamily {
        match self {
            SessionAction::Lock | SessionAction::LockAllSessions | SessionAction::ScreenOff => {
                ActionFamily::Locking
            }
            SessionAction::Sleep
            | SessionAction::Suspend
            | SessionAction::Hibernate
            | SessionAction::HybridSleep
            | SessionAction::SuspendThenHibernate => ActionFamily::Sleeping,
            SessionAction::Reboot
            | SessionAction::RebootToFirmwareSetup
            | SessionAction::RebootToBootLoaderMenu
            | SessionAction::Halt
            | SessionAction::PowerOff
            | SessionAction::CancelScheduledShutdown => ActionFamily::Shutdown,
            SessionAction::LogOut
            | SessionAction::TerminateSession
            | SessionAction::RestartShell => ActionFamily::Session,
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            SessionAction::Lock => "Lock",
            SessionAction::LockAllSessions => "Lock all sessions",
            SessionAction::ScreenOff => "Screen off",
            SessionAction::Sleep => "Sleep",
            SessionAction::Suspend => "Suspend",
            SessionAction::Hibernate => "Hibernate",
            SessionAction::HybridSleep => "Hybrid sleep",
            SessionAction::SuspendThenHibernate => "Suspend, then hibernate",
            SessionAction::Reboot => "Reboot",
            SessionAction::RebootToFirmwareSetup => "Reboot into firmware setup",
            SessionAction::RebootToBootLoaderMenu => "Reboot into boot menu",
            SessionAction::Halt => "Halt",
            SessionAction::PowerOff => "Power off",
            SessionAction::CancelScheduledShutdown => "Cancel scheduled shutdown",
            SessionAction::LogOut => "Log out",
            SessionAction::TerminateSession => "Terminate session",
            SessionAction::RestartShell => "Restart shell",
        }
    }

    pub fn mechanism(self) -> &'static str {
        match self {
            SessionAction::Lock => "logind LockSession",
            SessionAction::LockAllSessions => "logind LockSessions",
            SessionAction::ScreenOff => "niri PowerOffMonitors",
            SessionAction::Sleep => "logind Sleep",
            SessionAction::Suspend => "logind Suspend",
            SessionAction::Hibernate => "logind Hibernate",
            SessionAction::HybridSleep => "logind HybridSleep",
            SessionAction::SuspendThenHibernate => "logind SuspendThenHibernate",
            SessionAction::Reboot => "logind Reboot",
            SessionAction::RebootToFirmwareSetup => "logind SetRebootToFirmwareSetup + Reboot",
            SessionAction::RebootToBootLoaderMenu => "logind SetRebootToBootLoaderMenu + Reboot",
            SessionAction::Halt => "logind Halt",
            SessionAction::PowerOff => "logind PowerOff",
            SessionAction::CancelScheduledShutdown => "logind CancelScheduledShutdown",
            SessionAction::LogOut => "niri Quit",
            SessionAction::TerminateSession => "logind TerminateSession",
            SessionAction::RestartShell => "systemctl --user restart vetrata",
        }
    }

    pub fn glyph(self) -> Phosphor {
        let weight = Weight::Regular;
        match self {
            SessionAction::Lock => Phosphor::Lock(weight),
            SessionAction::LockAllSessions => Phosphor::LockKey(weight),
            SessionAction::ScreenOff => Phosphor::Monitor(weight),
            SessionAction::Sleep => Phosphor::Moon(weight),
            SessionAction::Suspend | SessionAction::SuspendThenHibernate => Phosphor::Bed(weight),
            SessionAction::Hibernate => Phosphor::Snowflake(weight),
            SessionAction::HybridSleep => Phosphor::CloudMoon(weight),
            SessionAction::Reboot => Phosphor::ArrowsClockwise(weight),
            SessionAction::RebootToFirmwareSetup => Phosphor::Cpu(weight),
            SessionAction::RebootToBootLoaderMenu => Phosphor::ListBullets(weight),
            SessionAction::Halt => Phosphor::Prohibit(weight),
            SessionAction::PowerOff => Phosphor::Power(weight),
            SessionAction::CancelScheduledShutdown => Phosphor::XCircle(weight),
            SessionAction::LogOut => Phosphor::SignOut(weight),
            SessionAction::TerminateSession => Phosphor::DoorOpen(weight),
            SessionAction::RestartShell => Phosphor::ArrowClockwise(weight),
        }
    }

    pub fn perform(self) {
        match self {
            SessionAction::Lock => logind::lock(),
            SessionAction::LockAllSessions => logind::lock_all_sessions(),
            SessionAction::ScreenOff => {
                niri::send_action_or_warn(&niri_ipc::Action::PowerOffMonitors {});
            }
            SessionAction::Sleep => logind::sleep(),
            SessionAction::Suspend => logind::suspend(),
            SessionAction::Hibernate => logind::hibernate(),
            SessionAction::HybridSleep => logind::hybrid_sleep(),
            SessionAction::SuspendThenHibernate => logind::suspend_then_hibernate(),
            SessionAction::Reboot => logind::reboot(),
            SessionAction::RebootToFirmwareSetup => logind::reboot_to_firmware_setup(),
            SessionAction::RebootToBootLoaderMenu => logind::reboot_to_boot_loader_menu(),
            SessionAction::Halt => logind::halt(),
            SessionAction::PowerOff => logind::power_off(),
            SessionAction::CancelScheduledShutdown => logind::cancel_scheduled_shutdown(),
            SessionAction::LogOut => niri::send_action_or_warn(&niri_ipc::Action::Quit {
                skip_confirmation: true,
            }),
            SessionAction::TerminateSession => logind::terminate_session(),
            SessionAction::RestartShell => {
                command::blocking::run("systemctl", &["--user", "restart", "vetrata.service"]);
            }
        }
    }
}
