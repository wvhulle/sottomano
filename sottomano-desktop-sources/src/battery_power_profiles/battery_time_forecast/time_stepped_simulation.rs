use std::{ops::ControlFlow, time::Duration};

use uom::si::{
    f64::{Energy, Power, Time},
    time::second,
};

use crate::battery_power_profiles::battery_time_forecast::Estimate;

const STEP: Duration = Duration::from_mins(1);
const GIVE_UP_AFTER: Duration = Duration::from_hours(72);
const DEVIATION_FADE: Duration = Duration::from_mins(15);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Scenario {
    Typical,
    Fading,
    Persisting,
}

impl Scenario {
    pub(super) fn deviation_after(self, now: Power, elapsed: Duration) -> Power {
        match self {
            Scenario::Typical => Power::default(),
            Scenario::Fading => now * (-elapsed.as_secs_f64() / DEVIATION_FADE.as_secs_f64()).exp(),
            Scenario::Persisting => now,
        }
    }
}

pub(super) fn time_to_move(
    energy: Energy,
    power_at: impl Fn(Duration, Energy) -> Option<Power>,
) -> Option<Duration> {
    let step = Time::new::<second>(STEP.as_secs_f64());
    let outcome = (0..)
        .map(|steps| STEP * steps)
        .take_while(|elapsed| *elapsed < GIVE_UP_AFTER)
        .try_fold(Energy::default(), |moved, elapsed| {
            let Some(power) = power_at(elapsed, moved).filter(|power| *power > Power::default())
            else {
                return ControlFlow::Break(None);
            };
            let after_step = moved + power * step;
            if after_step < energy {
                return ControlFlow::Continue(after_step);
            }
            let remainder = ((energy - moved) / power).get::<second>();
            ControlFlow::Break(Some(elapsed + Duration::from_secs_f64(remainder)))
        });
    match outcome {
        ControlFlow::Break(finished) => finished,
        ControlFlow::Continue(_) => None,
    }
}

pub(super) fn estimate(time_in: impl Fn(Scenario) -> Option<Duration>) -> Option<Estimate> {
    let fading = time_in(Scenario::Fading)?;
    let bounds = [Scenario::Typical, Scenario::Persisting].map(time_in);
    let earliest = bounds.into_iter().flatten().fold(fading, Duration::min);
    let slowest = bounds
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .map(|bounds| bounds.into_iter().fold(fading, Duration::max));
    Some(Estimate {
        time: whole_minutes(fading),
        earliest: whole_minutes(earliest),
        latest: slowest.map(whole_minutes),
    })
}

fn whole_minutes(duration: Duration) -> Duration {
    Duration::from_mins((duration.as_secs_f64() / 60.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use uom::si::{energy::watt_hour, power::watt};

    use super::*;

    fn hours(duration: Duration) -> f64 {
        duration.as_secs_f64() / 3600.0
    }

    #[test]
    fn constant_power_moves_energy_over_power() {
        let time = time_to_move(Energy::new::<watt_hour>(30.0), |_, _| {
            Some(Power::new::<watt>(10.0))
        })
        .expect("finishes");
        assert!((hours(time) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn stalled_power_never_finishes() {
        assert!(
            time_to_move(Energy::new::<watt_hour>(30.0), |_, _| {
                Some(Power::new::<watt>(0.0))
            })
            .is_none()
        );
    }

    #[test]
    fn trickle_beyond_the_cap_never_finishes() {
        assert!(
            time_to_move(Energy::new::<watt_hour>(100.0), |_, _| {
                Some(Power::new::<watt>(0.1))
            })
            .is_none()
        );
    }

    #[test]
    fn fading_deviation_lies_between_typical_and_persisting() {
        let now = Power::new::<watt>(-10.0);
        let after = Duration::from_mins(10);
        let fading = Scenario::Fading.deviation_after(now, after);
        assert!(Scenario::Persisting.deviation_after(now, after) < fading);
        assert!(fading < Scenario::Typical.deviation_after(now, after));
    }

    #[test]
    fn unfinished_scenario_leaves_the_estimate_unbounded_above() {
        let estimate = estimate(|scenario| match scenario {
            Scenario::Typical => Some(Duration::from_mins(30)),
            Scenario::Fading => Some(Duration::from_mins(60)),
            Scenario::Persisting => None,
        })
        .expect("estimate");
        assert_eq!(estimate.earliest, Duration::from_mins(30));
        assert_eq!(estimate.time, Duration::from_mins(60));
        assert!(estimate.latest.is_none());
    }
}
