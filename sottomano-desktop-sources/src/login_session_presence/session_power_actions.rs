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

    #[must_use]
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

    #[must_use]
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

    #[must_use]
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
            SessionAction::RestartShell => "systemctl --user restart sottomano",
        }
    }
}
