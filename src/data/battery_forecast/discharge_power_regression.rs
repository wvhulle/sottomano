use std::time::Duration;

use adskalman::{KalmanFilterNoControl, StateAndCovariance};
use nalgebra::{Matrix1x3, Matrix3, Vector1, Vector3};

use crate::data::{
    battery_forecast::{
        autocorrelated_residual::ResidualProcess,
        normally_distributed_power::NormalPower,
        random_walk_kalman_models::{LinearObservation, RandomWalk},
    },
    source::{Source, power_profile::Conditions, sysfs_sensors::battery::Battery},
};

const BASE_DRIFT_W2_PER_S: f64 = 2e-7;
const COEFFICIENT_DRIFT_W2_PER_S: f64 = 1e-7;
const SEED_VARIANCE_BASE: f64 = 100.0;
const SEED_VARIANCE_COEFF: f64 = 400.0;

fn regressors(conditions: Conditions) -> Matrix1x3<f64> {
    Matrix1x3::new(1.0, conditions.brightness_frac, conditions.cpu_busy_frac)
}

fn coefficient_drift() -> RandomWalk<3> {
    RandomWalk::over(
        Battery::INTERVAL,
        Vector3::new(
            BASE_DRIFT_W2_PER_S,
            COEFFICIENT_DRIFT_W2_PER_S,
            COEFFICIENT_DRIFT_W2_PER_S,
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionState {
    pub coefficients: Vector3<f64>,
    pub covariance: Matrix3<f64>,
    pub residuals: ResidualProcess,
}

impl RegressionState {
    pub fn seed(first_draw_w: f64) -> Self {
        RegressionState {
            coefficients: Vector3::new(first_draw_w, 0.0, 0.0),
            covariance: Matrix3::from_diagonal(&Vector3::new(
                SEED_VARIANCE_BASE,
                SEED_VARIANCE_COEFF,
                SEED_VARIANCE_COEFF,
            )),
            residuals: ResidualProcess::seed(),
        }
    }

    pub fn observe(self, conditions: Conditions, observed_w: f64) -> Self {
        let drift = coefficient_drift();
        let prior_fit = RegressionState {
            covariance: self.covariance + drift.drift_covariance,
            ..self
        }
        .fit(conditions);
        let residuals = self
            .residuals
            .absorb(observed_w - prior_fit.mean_w, prior_fit.variance);
        let observation = LinearObservation::new(
            regressors(conditions),
            self.residuals.variance_per_independent_sample(),
        );
        let filter = KalmanFilterNoControl::new(&drift, &observation);
        let prior = StateAndCovariance::new(self.coefficients, self.covariance);
        let unfitted = RegressionState { residuals, ..self };
        filter
            .step(&prior, &Vector1::new(observed_w))
            .map_or(unfitted, |posterior| RegressionState {
                coefficients: *posterior.state(),
                covariance: *posterior.covariance(),
                residuals,
            })
    }

    pub fn interrupted(self) -> Self {
        RegressionState {
            residuals: self.residuals.interrupted(),
            ..self
        }
    }

    pub(super) fn fit(&self, conditions: Conditions) -> NormalPower {
        let row = regressors(conditions);
        NormalPower {
            mean_w: (row * self.coefficients)[0],
            variance: (row * self.covariance * row.transpose())[(0, 0)],
        }
    }

    pub(super) fn average_draw_over(
        &self,
        horizon: Duration,
        conditions: Conditions,
        current_w: Option<f64>,
    ) -> NormalPower {
        let fit = self.fit(conditions);
        let steps = (horizon.as_secs_f64() / Battery::INTERVAL.as_secs_f64()).max(1.0);
        let current_residual = current_w.map(|draw| draw - fit.mean_w);
        fit.plus(self.residuals.average_over(steps, current_residual))
    }
}

#[cfg(test)]
pub(super) fn trained(
    conditions: Conditions,
    draws: impl IntoIterator<Item = f64>,
) -> RegressionState {
    draws
        .into_iter()
        .fold(RegressionState::seed(10.0), |model, draw| {
            model.observe(conditions, draw)
        })
}

#[cfg(test)]
pub(super) fn slow_wave(samples: usize) -> impl Iterator<Item = f64> {
    use std::f64::consts::TAU;
    (0..samples).map(|i| 10.0 + 2.0 * (i as f64 * TAU / 200.0).sin())
}

#[cfg(test)]
mod tests {
    use std::iter::repeat_n;

    use super::*;

    #[test]
    fn keeps_learned_draw_through_a_short_burst() {
        let conditions = Conditions::new(0.5, 0.3);
        let settled = trained(conditions, repeat_n(10.0, 2000));
        let after_burst = (0..12).fold(settled, |model, _| model.observe(conditions, 20.0));
        assert!(
            after_burst.fit(conditions).mean_w < 11.0,
            "one minute of heavy load should not overwrite hours of history: {}",
            after_burst.fit(conditions).mean_w
        );
    }

    #[test]
    fn slowly_wandering_draw_is_autocorrelated() {
        let model = trained(Conditions::new(0.5, 0.3), slow_wave(3000));
        assert!(
            model.residuals.autocorrelation() > 0.9,
            "{}",
            model.residuals.autocorrelation()
        );
    }

    #[test]
    fn separates_brightness_from_base_draw() {
        let true_draw = |c: Conditions| 5.0 + 20.0 * c.brightness_frac;
        let dim = Conditions::new(0.1, 0.0);
        let bright = Conditions::new(0.9, 0.0);
        let model = (0..80).fold(RegressionState::seed(true_draw(dim)), |model, _| {
            model
                .observe(dim, true_draw(dim))
                .observe(bright, true_draw(bright))
        });
        assert!(model.fit(bright).mean_w > model.fit(dim).mean_w + 10.0);
    }
}
