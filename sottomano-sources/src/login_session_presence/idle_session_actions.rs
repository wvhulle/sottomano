use crate::login_session_presence::session_power_actions::SessionAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdleAction {
    Lock,
    ScreenOff,
    Sleep,
    PowerOff,
}

impl IdleAction {
    pub const ALL: [IdleAction; 4] = [Self::Lock, Self::ScreenOff, Self::Sleep, Self::PowerOff];

    #[must_use]
    pub fn session_action(self) -> SessionAction {
        match self {
            IdleAction::Lock => SessionAction::Lock,
            IdleAction::ScreenOff => SessionAction::ScreenOff,
            IdleAction::Sleep => SessionAction::Sleep,
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
    fn no_idle_action_names_a_specific_sleep_state() {
        let specific_sleep_states = [
            SessionAction::Suspend,
            SessionAction::Hibernate,
            SessionAction::HybridSleep,
            SessionAction::SuspendThenHibernate,
        ];
        for state in specific_sleep_states {
            assert_eq!(IdleAction::performing(state), None, "{state:?}");
        }
    }
}
