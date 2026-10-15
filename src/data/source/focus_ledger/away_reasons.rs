use super::focus_units::Presence;
use crate::data::source::{idle_notify::IdleEdge, logind::SleepEdge};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenEdge {
    Off,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwayReasonEdge {
    IdleNotifierEdge(IdleEdge),
    LogindSleepEdge(SleepEdge),
    ScreenPowerEdge(ScreenEdge),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatestAwayReasonEdges {
    latest_idle_edge: IdleEdge,
    latest_sleep_edge: SleepEdge,
    latest_screen_power_edge: ScreenEdge,
}

impl Default for LatestAwayReasonEdges {
    fn default() -> Self {
        Self {
            latest_idle_edge: IdleEdge::Resumed,
            latest_sleep_edge: SleepEdge::Resumed,
            latest_screen_power_edge: ScreenEdge::On,
        }
    }
}

impl LatestAwayReasonEdges {
    pub(super) fn with_latest_edge_of_its_source(self, edge: AwayReasonEdge) -> Self {
        match edge {
            AwayReasonEdge::IdleNotifierEdge(latest_idle_edge) => Self {
                latest_idle_edge,
                ..self
            },
            AwayReasonEdge::LogindSleepEdge(latest_sleep_edge) => Self {
                latest_sleep_edge,
                ..self
            },
            AwayReasonEdge::ScreenPowerEdge(latest_screen_power_edge) => Self {
                latest_screen_power_edge,
                ..self
            },
        }
    }

    pub(super) fn most_away_presence(&self) -> Presence {
        if self.latest_sleep_edge == SleepEdge::Suspending {
            Presence::Asleep
        } else if self.latest_screen_power_edge == ScreenEdge::Off {
            Presence::ScreenOff
        } else if self.latest_idle_edge == IdleEdge::Idled {
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
        let idled = AwayReasonEdge::IdleNotifierEdge(IdleEdge::Idled);
        let screen_off = AwayReasonEdge::ScreenPowerEdge(ScreenEdge::Off);
        let suspending = AwayReasonEdge::LogindSleepEdge(SleepEdge::Suspending);
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
                AwayReasonEdge::ScreenPowerEdge(ScreenEdge::On),
                AwayReasonEdge::IdleNotifierEdge(IdleEdge::Resumed),
            ]),
            Presence::Asleep
        );
    }
}
