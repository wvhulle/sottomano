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

    #[must_use]
    pub fn performing(action: SessionAction) -> Option<IdleAction> {
        Self::ALL
            .into_iter()
            .find(|idle| idle.session_action() == action)
    }
}

#[cfg(test)]
mod tests {
    use super::IdleAction;
    use crate::login_session_presence::session_power_actions::SessionAction;

    #[test]
    fn idle_sleep_leaves_the_sleep_state_to_logind_sleep_operation() {
        assert_eq!(IdleAction::Sleep.session_action(), SessionAction::Sleep);
    }

    #[test]
    fn idle_hibernate_asks_logind_for_hibernation_by_name() {
        assert_eq!(
            IdleAction::Hibernate.session_action(),
            SessionAction::Hibernate
        );
    }

    #[test]
    fn hibernate_is_the_only_idle_action_naming_a_specific_sleep_state() {
        let specific_sleep_states = [
            SessionAction::Suspend,
            SessionAction::HybridSleep,
            SessionAction::SuspendThenHibernate,
        ];
        for state in specific_sleep_states {
            assert_eq!(IdleAction::performing(state), None, "{state:?}");
        }
    }
}
