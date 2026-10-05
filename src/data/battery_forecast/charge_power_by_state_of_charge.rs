#[cfg(test)]
use std::iter::repeat_n;

use adskalman::{KalmanFilterNoControl, StateAndCovariance};
use nalgebra::{Matrix1, SMatrix, Vector1};
use uom::si::{
    f64::{Power, Ratio},
    power::watt,
    ratio::ratio,
};

use crate::data::{
    battery_forecast::{
        autocorrelated_residual::ResidualProcess,
        normally_distributed_power::NormalPower,
        random_walk_kalman_models::{LinearObservation, RandomWalk},
    },
    source::{Source, sysfs_sensors::battery::Battery},
};

pub const CHARGE_BINS: usize = 20;
const BIN_DRIFT_W2_PER_S: f64 = 2e-7;
const SEED_BIN_VARIANCE: f64 = 100.0;

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

    fn as_normal(self) -> NormalPower {
        NormalPower::from_watts(self.mean_w, self.variance)
    }
}

pub fn bin_of(state_of_charge: Ratio) -> usize {
    let fraction = state_of_charge.get::<ratio>().clamp(0.0, 1.0);
    ((fraction * CHARGE_BINS as f64) as usize).min(CHARGE_BINS - 1)
}

pub(super) fn bin_lower_edge(bin: usize) -> Ratio {
    Ratio::new::<ratio>(bin as f64 / CHARGE_BINS as f64)
}

fn bin_drift() -> RandomWalk<1> {
    RandomWalk::over(Battery::INTERVAL, Vector1::new(BIN_DRIFT_W2_PER_S))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChargeCurve {
    pub bins: [Option<PowerBin>; CHARGE_BINS],
    pub residuals: ResidualProcess,
}

impl Default for ChargeCurve {
    fn default() -> Self {
        ChargeCurve {
            bins: [None; CHARGE_BINS],
            residuals: ResidualProcess::seed(),
        }
    }
}

impl ChargeCurve {
    pub fn observe(self, state_of_charge: Ratio, observed: Power) -> Self {
        let observed_w = observed.get::<watt>();
        let bin = bin_of(state_of_charge);
        let prior = self.bins[bin].unwrap_or_else(|| PowerBin::seeded_at(observed_w));
        let drift = bin_drift();
        let drifted_variance = prior.variance + drift.drift_covariance[(0, 0)];
        let residuals = self
            .residuals
            .absorb(observed_w - prior.mean_w, drifted_variance);
        let observation = LinearObservation::new(
            SMatrix::<f64, 1, 1>::identity(),
            self.residuals.variance_per_independent_sample(),
        );
        let filter = KalmanFilterNoControl::new(&drift, &observation);
        let posterior = filter
            .step(
                &StateAndCovariance::new(Vector1::new(prior.mean_w), Matrix1::new(prior.variance)),
                &Vector1::new(observed_w),
            )
            .map_or(prior, |posterior| PowerBin {
                mean_w: posterior.state()[0],
                variance: posterior.covariance()[(0, 0)],
            });
        let mut bins = self.bins;
        bins[bin] = Some(posterior);
        ChargeCurve { bins, residuals }
    }

    pub fn has_history(&self) -> bool {
        self.bins.iter().any(Option::is_some)
    }

    pub fn interrupted(self) -> Self {
        ChargeCurve {
            residuals: self.residuals.interrupted(),
            ..self
        }
    }

    pub(super) fn power_in(&self, bin: usize) -> Option<NormalPower> {
        self.bins[bin].map(PowerBin::as_normal).or_else(|| {
            self.nearest_learned(bin).map(|nearest| {
                NormalPower::from_watts(nearest.mean_w, nearest.variance + SEED_BIN_VARIANCE)
            })
        })
    }

    fn nearest_learned(&self, bin: usize) -> Option<PowerBin> {
        (1..CHARGE_BINS).find_map(|distance| {
            bin.checked_sub(distance)
                .and_then(|below| self.bins[below])
                .or_else(|| self.bins.get(bin + distance).copied().flatten())
        })
    }
}

#[cfg(test)]
pub(super) fn learned_curve(power_at: impl Fn(Ratio) -> Power) -> ChargeCurve {
    let half_bin = Ratio::new::<ratio>(0.5 / CHARGE_BINS as f64);
    (0..CHARGE_BINS).flat_map(|bin| repeat_n(bin, 200)).fold(
        ChargeCurve::default(),
        |curve, bin| {
            let state_of_charge = bin_lower_edge(bin) + half_bin;
            curve.observe(state_of_charge, power_at(state_of_charge))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_cover_the_whole_range() {
        assert_eq!(bin_of(Ratio::new::<ratio>(0.0)), 0);
        assert_eq!(bin_of(Ratio::new::<ratio>(0.049)), 0);
        assert_eq!(bin_of(Ratio::new::<ratio>(0.05)), 1);
        assert_eq!(bin_of(Ratio::new::<ratio>(1.0)), CHARGE_BINS - 1);
    }

    #[test]
    fn learns_a_taper_per_bin() {
        let curve = learned_curve(|soc| {
            if soc < Ratio::new::<ratio>(0.8) {
                Power::new::<watt>(30.0)
            } else {
                Power::new::<watt>(10.0)
            }
        });
        let mean = |fraction| {
            curve
                .power_in(bin_of(Ratio::new::<ratio>(fraction)))
                .expect("learned")
                .mean
                .get::<watt>()
        };
        assert!((mean(0.5) - 30.0).abs() < 0.5, "{}", mean(0.5));
        assert!((mean(0.9) - 10.0).abs() < 0.5, "{}", mean(0.9));
    }

    #[test]
    fn unobserved_bin_borrows_its_neighbour_with_more_doubt() {
        let curve = ChargeCurve::default()
            .observe(Ratio::new::<ratio>(0.52), Power::new::<watt>(25.0))
            .observe(Ratio::new::<ratio>(0.52), Power::new::<watt>(25.0));
        let observed = curve
            .power_in(bin_of(Ratio::new::<ratio>(0.52)))
            .expect("observed");
        let borrowed = curve
            .power_in(bin_of(Ratio::new::<ratio>(0.9)))
            .expect("borrowed");
        assert!((borrowed.mean - observed.mean).get::<watt>().abs() < 1e-9);
        assert!(borrowed.spread > observed.spread);
    }

    #[test]
    fn empty_curve_knows_nothing() {
        assert!(ChargeCurve::default().power_in(3).is_none());
    }
}
