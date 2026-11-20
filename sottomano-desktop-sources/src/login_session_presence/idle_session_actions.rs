use crate::login_session_presence::session_power_actions::SessionAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdleAction {
    Lock,
    ScreenOff,
    Sleep,
    Hibernate,
    PowerOff,
}

impl IdleAction {
    pub const ALL: [IdleAction; 5] = [
        Self::Lock,
        Self::ScreenOff,
        Self::Sleep,
        Self::Hibernate,
        Self::PowerOff,
    ];

    #[must_use]
    pub fn session_action(self) -> SessionAction {
        match self {
            IdleAction::Lock => SessionAction::Lock,
            IdleAction::ScreenOff => SessionAction::ScreenOff,
            IdleAction::Sleep => SessionAction::Sleep,
            IdleAction::Hibernate => SessionAction::Hibernate,
            IdleAction::PowerOff => SessionAction::PowerOff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IdleAction;
    use crate::login_session_presence::session_power_actions::SessionAction;

    #[test]
    fn idle_hibernate_asks_logind_for_hibernation_by_name() {
        assert_eq!(
            IdleAction::Hibernate.session_action(),
            SessionAction::Hibernate
        );
    }
}
