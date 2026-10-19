use super::focus_units::Presence;
use crate::login_session_presence::{logind_manager::SleepEdge, wayland_idle_notify::IdleEdge};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenEdge {
    Off,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwayReasonEdge {
    IdleNotifier(IdleEdge),
    LogindSleep(SleepEdge),
    ScreenPower(ScreenEdge),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatestAwayReasonEdges {
    idle: IdleEdge,
    sleep: SleepEdge,
    screen_power: ScreenEdge,
}

impl Default for LatestAwayReasonEdges {
    fn default() -> Self {
        Self {
            idle: IdleEdge::Resumed,
            sleep: SleepEdge::Resumed,
            screen_power: ScreenEdge::On,
        }
    }
}

impl LatestAwayReasonEdges {
    pub(super) fn with_latest_edge_of_its_source(self, edge: AwayReasonEdge) -> Self {
        match edge {
            AwayReasonEdge::IdleNotifier(idle) => Self { idle, ..self },
            AwayReasonEdge::LogindSleep(sleep) => Self { sleep, ..self },
            AwayReasonEdge::ScreenPower(screen_power) => Self {
                screen_power,
                ..self
            },
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
    use super::*;

    fn presence_after(edges: &[AwayReasonEdge]) -> Presence {
        edges
            .iter()
            .copied()
            .fold(
                LatestAwayReasonEdges::default(),
                LatestAwayReasonEdges::with_latest_edge_of_its_source,
            )
            .most_away_presence()
    }

    #[test]
    fn the_most_away_reason_wins_regardless_of_arrival_order() {
        let idled = AwayReasonEdge::IdleNotifier(IdleEdge::Idled);
        let screen_off = AwayReasonEdge::ScreenPower(ScreenEdge::Off);
        let suspending = AwayReasonEdge::LogindSleep(SleepEdge::Suspending);
        assert_eq!(presence_after(&[]), Presence::Active);
        assert_eq!(presence_after(&[idled]), Presence::Idle);
        assert_eq!(presence_after(&[screen_off, idled]), Presence::ScreenOff);
        assert_eq!(presence_after(&[idled, screen_off]), Presence::ScreenOff);
        assert_eq!(
            presence_after(&[suspending, screen_off, idled]),
            Presence::Asleep
        );
        assert_eq!(
            presence_after(&[
                suspending,
                AwayReasonEdge::ScreenPower(ScreenEdge::On),
                AwayReasonEdge::IdleNotifier(IdleEdge::Resumed),
            ]),
            Presence::Asleep
        );
    }
}
