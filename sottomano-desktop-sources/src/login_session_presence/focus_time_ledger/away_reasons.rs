use super::{FocusSignal, focus_units::Presence};
use crate::login_session_presence::{logind_manager::SleepEdge, wayland_idle_notify::IdleEdge};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenEdge {
    Off,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatestAwayReasons {
    idle: IdleEdge,
    sleep: SleepEdge,
    screen_power: ScreenEdge,
}

impl Default for LatestAwayReasons {
    fn default() -> Self {
        Self {
            idle: IdleEdge::Resumed,
            sleep: SleepEdge::Resumed,
            screen_power: ScreenEdge::On,
        }
    }
}

impl LatestAwayReasons {
    pub(super) fn note_signal(&mut self, signal: &FocusSignal) {
        match *signal {
            FocusSignal::Idle(idle) => self.idle = idle,
            FocusSignal::Sleep(sleep) => self.sleep = sleep,
            FocusSignal::ScreenPower(screen_power) => self.screen_power = screen_power,
            FocusSignal::WindowFocused(_) | FocusSignal::NoWindowFocused => {}
        }
    }

    pub(super) fn most_away_presence(self) -> Presence {
        if self.sleep == SleepEdge::Suspending {
            Presence::Asleep
        } else if self.screen_power == ScreenEdge::Off {
            Presence::ScreenOff
        } else if self.idle == IdleEdge::Idled {
            Presence::Idle
        } else {
            Presence::Active
        }
    }
}

#[cfg(test)]
mod tests {
    use std::slice::from_ref;

    use super::*;

    fn presence_after(signals: &[FocusSignal]) -> Presence {
        let mut latest_away_reasons = LatestAwayReasons::default();
        for signal in signals {
            latest_away_reasons.note_signal(signal);
        }
        latest_away_reasons.most_away_presence()
    }

    #[test]
    fn the_most_away_reason_wins_regardless_of_arrival_order() {
        let idled = FocusSignal::Idle(IdleEdge::Idled);
        let screen_off = FocusSignal::ScreenPower(ScreenEdge::Off);
        let suspending = FocusSignal::Sleep(SleepEdge::Suspending);
        assert_eq!(presence_after(&[]), Presence::Active);
        assert_eq!(presence_after(from_ref(&idled)), Presence::Idle);
        assert_eq!(
            presence_after(&[screen_off.clone(), idled.clone()]),
            Presence::ScreenOff
        );
        assert_eq!(
            presence_after(&[idled.clone(), screen_off.clone()]),
            Presence::ScreenOff
        );
        assert_eq!(
            presence_after(&[suspending.clone(), screen_off, idled]),
            Presence::Asleep
        );
        assert_eq!(
            presence_after(&[
                suspending,
                FocusSignal::ScreenPower(ScreenEdge::On),
                FocusSignal::Idle(IdleEdge::Resumed),
            ]),
            Presence::Asleep
        );
    }
}
