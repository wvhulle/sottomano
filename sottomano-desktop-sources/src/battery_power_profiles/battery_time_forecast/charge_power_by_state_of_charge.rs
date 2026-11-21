#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveHistory {
    Recorded,
    Empty,
}

#[cfg(test)]
use std::iter::repeat_n;
use std::time::Duration;

use uom::si::{
    f64::{Power, Ratio},
    power::watt,
    ratio::ratio,
};

#[cfg(test)]
use crate::battery_power_profiles::upower_battery_state::BATTERY_POLL_INTERVAL;

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
pub fn charge_bin_index(state_of_charge: Ratio) -> usize {
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
        let bin = charge_bin_index(state_of_charge);
        let prior = self.bins[bin].unwrap_or_else(|| PowerBin::seeded_at(observed_w));
        let mut bins = self.bins;
        bins[bin] = Some(prior.updated_with(observed_w, since_previous));
        ChargeCurve { bins }
    }

    #[must_use]
    pub fn history(&self) -> CurveHistory {
        if self.bins.iter().any(Option::is_some) {
            CurveHistory::Recorded
        } else {
            CurveHistory::Empty
        }
    }

    pub(super) fn learned(&self) -> Option<Vec<Power>> {
        (0..CHARGE_BINS)
            .map(|bin| self.nearest_learned(bin))
            .collect()
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

    fn fraction_ratio(fraction: f64) -> Ratio {
        Ratio::new::<ratio>(fraction)
    }

    #[test]
    fn bins_cover_the_whole_range() {
        assert_eq!(charge_bin_index(fraction_ratio(0.0)), 0);
        assert_eq!(charge_bin_index(fraction_ratio(0.049)), 0);
        assert_eq!(charge_bin_index(fraction_ratio(0.05)), 1);
        assert_eq!(charge_bin_index(fraction_ratio(1.0)), CHARGE_BINS - 1);
    }

    #[test]
    fn learns_a_taper_per_bin() {
        let curve = learned_curve(|soc| {
            if soc < fraction_ratio(0.8) {
                Power::new::<watt>(30.0)
            } else {
                Power::new::<watt>(10.0)
            }
        })
        .learned()
        .expect("learned");
        let watts_at = |fraction| curve[charge_bin_index(fraction_ratio(fraction))].get::<watt>();
        assert!((watts_at(0.5) - 30.0).abs() < 0.5, "{}", watts_at(0.5));
        assert!((watts_at(0.9) - 10.0).abs() < 0.5, "{}", watts_at(0.9));
    }

    #[test]
    fn unobserved_bin_borrows_its_neighbour() {
        let curve = ChargeCurve::default()
            .observe(
                fraction_ratio(0.52),
                Power::new::<watt>(25.0),
                BATTERY_POLL_INTERVAL,
            )
            .learned()
            .expect("learned");
        assert_eq!(curve[charge_bin_index(fraction_ratio(0.9))], curve[charge_bin_index(fraction_ratio(0.52))]);
    }

    #[test]
    fn empty_curve_knows_nothing() {
        assert!(ChargeCurve::default().learned().is_none());
        assert_eq!(ChargeCurve::default().history(), CurveHistory::Empty);
    }
}
