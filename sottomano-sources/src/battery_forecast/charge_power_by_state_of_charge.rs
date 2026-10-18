#[cfg(test)]
use std::iter::repeat_n;
use std::time::Duration;

use uom::si::{
    f64::{Power, Ratio},
    power::watt,
    ratio::ratio,
};

#[cfg(test)]
use crate::sysfs_sensors::battery::BATTERY_POLL_INTERVAL;

pub const CHARGE_BINS: usize = 20;
const BIN_DRIFT_W2_PER_S: f64 = 2e-7;
const SEED_BIN_VARIANCE: f64 = 100.0;
const MEASUREMENT_VARIANCE_W2: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerBin {
    pub mean_w: f64,
    pub variance: f64,
}

impl PowerBin {
    fn seeded_at(observed_w: f64) -> Self {
        PowerBin {
            mean_w: observed_w,
            variance: SEED_BIN_VARIANCE,
        }
    }

    fn updated_with(self, observed_w: f64, since_previous: Duration) -> Self {
        let drifted = self.variance + BIN_DRIFT_W2_PER_S * since_previous.as_secs_f64();
        let gain = drifted / (drifted + MEASUREMENT_VARIANCE_W2);
        PowerBin {
            mean_w: self.mean_w + gain * (observed_w - self.mean_w),
            variance: (1.0 - gain) * drifted,
        }
    }
}

#[must_use]
pub fn bin_of(state_of_charge: Ratio) -> usize {
    let fraction = state_of_charge.get::<ratio>().clamp(0.0, 1.0);
    ((fraction * CHARGE_BINS as f64) as usize).min(CHARGE_BINS - 1)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChargeCurve {
    pub bins: [Option<PowerBin>; CHARGE_BINS],
}

impl ChargeCurve {
    #[must_use]
    pub fn observe(
        self,
        state_of_charge: Ratio,
        observed: Power,
        since_previous: Duration,
    ) -> Self {
        let observed_w = observed.get::<watt>();
        let bin = bin_of(state_of_charge);
        let prior = self.bins[bin].unwrap_or_else(|| PowerBin::seeded_at(observed_w));
        let mut bins = self.bins;
        bins[bin] = Some(prior.updated_with(observed_w, since_previous));
        ChargeCurve { bins }
    }

    pub fn has_history(&self) -> bool {
        self.bins.iter().any(Option::is_some)
    }

    pub(super) fn learned(&self) -> Option<LearnedCurve> {
        (0..CHARGE_BINS)
            .map(|bin| self.nearest_learned(bin))
            .collect::<Option<Vec<_>>>()
            .map(LearnedCurve)
    }

    fn nearest_learned(&self, bin: usize) -> Option<Power> {
        (0..CHARGE_BINS)
            .find_map(|distance| {
                bin.checked_sub(distance)
                    .and_then(|below| self.bins[below])
                    .or_else(|| self.bins.get(bin + distance).copied().flatten())
            })
            .map(|learned| Power::new::<watt>(learned.mean_w))
    }
}

pub(super) struct LearnedCurve(Vec<Power>);

impl LearnedCurve {
    pub(super) fn power_at(&self, state_of_charge: Ratio) -> Power {
        self.0[bin_of(state_of_charge)]
    }
}

#[cfg(test)]
pub(super) fn learned_curve(power_at: impl Fn(Ratio) -> Power) -> ChargeCurve {
    let half_bin = Ratio::new::<ratio>(0.5 / CHARGE_BINS as f64);
    (0..CHARGE_BINS).flat_map(|bin| repeat_n(bin, 200)).fold(
        ChargeCurve::default(),
        |curve, bin| {
            let state_of_charge = Ratio::new::<ratio>(bin as f64 / CHARGE_BINS as f64) + half_bin;
            curve.observe(
                state_of_charge,
                power_at(state_of_charge),
                BATTERY_POLL_INTERVAL,
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ratio_of(fraction: f64) -> Ratio {
        Ratio::new::<ratio>(fraction)
    }

    #[test]
    fn bins_cover_the_whole_range() {
        assert_eq!(bin_of(ratio_of(0.0)), 0);
        assert_eq!(bin_of(ratio_of(0.049)), 0);
        assert_eq!(bin_of(ratio_of(0.05)), 1);
        assert_eq!(bin_of(ratio_of(1.0)), CHARGE_BINS - 1);
    }

    #[test]
    fn learns_a_taper_per_bin() {
        let curve = learned_curve(|soc| {
            if soc < ratio_of(0.8) {
                Power::new::<watt>(30.0)
            } else {
                Power::new::<watt>(10.0)
            }
        })
        .learned()
        .expect("learned");
        let watts_at = |fraction| curve.power_at(ratio_of(fraction)).get::<watt>();
        assert!((watts_at(0.5) - 30.0).abs() < 0.5, "{}", watts_at(0.5));
        assert!((watts_at(0.9) - 10.0).abs() < 0.5, "{}", watts_at(0.9));
    }

    #[test]
    fn unobserved_bin_borrows_its_neighbour() {
        let curve = ChargeCurve::default()
            .observe(
                ratio_of(0.52),
                Power::new::<watt>(25.0),
                BATTERY_POLL_INTERVAL,
            )
            .learned()
            .expect("learned");
        assert_eq!(
            curve.power_at(ratio_of(0.9)),
            curve.power_at(ratio_of(0.52))
        );
    }

    #[test]
    fn empty_curve_knows_nothing() {
        assert!(ChargeCurve::default().learned().is_none());
        assert!(!ChargeCurve::default().has_history());
    }
}
