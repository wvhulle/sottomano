use std::time::Duration;

use adskalman::{KalmanFilterNoControl, StateAndCovariance};
use nalgebra::{Matrix1x3, Matrix3, Vector1, Vector3};
use uom::si::{f64::Power, power::watt};

#[cfg(test)]
use crate::battery_power_profiles::upower_battery_state::BATTERY_POLL_INTERVAL;
use crate::battery_power_profiles::{
    battery_time_forecast::random_walk_kalman_models::{LinearObservation, RandomWalk},
    power_profiles_daemon::Conditions,
};

const BASE_DRIFT_W2_PER_S: f64 = 2e-7;
const COEFFICIENT_DRIFT_W2_PER_S: f64 = 1e-7;
const SEED_VARIANCE_BASE: f64 = 100.0;
const SEED_VARIANCE_COEFF: f64 = 400.0;
const MEASUREMENT_VARIANCE_W2: f64 = 4.0;

fn regressors(conditions: Conditions) -> Matrix1x3<f64> {
    Matrix1x3::new(1.0, conditions.brightness_frac, conditions.cpu_busy_frac)
}

fn coefficient_drift(since_previous: Duration) -> RandomWalk<3> {
    RandomWalk::over(
        since_previous,
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
}

impl RegressionState {
    #[must_use]
    pub fn seed(first_draw: Power) -> Self {
        RegressionState {
            coefficients: Vector3::new(first_draw.get::<watt>(), 0.0, 0.0),
            covariance: Matrix3::from_diagonal(&Vector3::new(
                SEED_VARIANCE_BASE,
                SEED_VARIANCE_COEFF,
                SEED_VARIANCE_COEFF,
            )),
        }
    }

    #[must_use]
    pub fn observe(
        self,
        conditions: Conditions,
        observed: Power,
        since_previous: Duration,
    ) -> Self {
        let drift = coefficient_drift(since_previous);
        let observation = LinearObservation::new(regressors(conditions), MEASUREMENT_VARIANCE_W2);
        let filter = KalmanFilterNoControl::new(&drift, &observation);
        let prior = StateAndCovariance::new(self.coefficients, self.covariance);
        filter
            .step(&prior, &Vector1::new(observed.get::<watt>()))
            .map_or(self, |posterior| RegressionState {
                coefficients: *posterior.state(),
                covariance: *posterior.covariance(),
            })
    }

    #[must_use]
    pub fn expected_draw(&self, conditions: Conditions) -> Power {
        Power::new::<watt>((regressors(conditions) * self.coefficients)[0])
    }
}

#[cfg(test)]
pub(super) fn trained(
    conditions: Conditions,
    draws: impl IntoIterator<Item = Power>,
) -> RegressionState {
    draws.into_iter().fold(
        RegressionState::seed(Power::new::<watt>(10.0)),
        |model, draw| model.observe(conditions, draw, BATTERY_POLL_INTERVAL),
    )
}

#[cfg(test)]
mod tests {
    use std::iter::repeat_n;

    use super::*;

    #[test]
    fn keeps_learned_draw_through_a_short_burst() {
        let conditions = Conditions::new(0.5, 0.3);
        let settled = trained(conditions, repeat_n(Power::new::<watt>(10.0), 2000));
        let after_burst = (0..12).fold(settled, |model, _| {
            model.observe(conditions, Power::new::<watt>(20.0), BATTERY_POLL_INTERVAL)
        });
        assert!(
            after_burst.expected_draw(conditions) < Power::new::<watt>(11.0),
            "one minute of heavy load should not overwrite hours of history: {:?}",
            after_burst.expected_draw(conditions)
        );
    }

    #[test]
    fn separates_brightness_from_base_draw() {
        let true_draw = |c: Conditions| Power::new::<watt>(5.0 + 20.0 * c.brightness_frac);
        let dim = Conditions::new(0.1, 0.0);
        let bright = Conditions::new(0.9, 0.0);
        let model = (0..80).fold(RegressionState::seed(true_draw(dim)), |model, _| {
            model
                .observe(dim, true_draw(dim), BATTERY_POLL_INTERVAL)
                .observe(bright, true_draw(bright), BATTERY_POLL_INTERVAL)
        });
        assert!(model.expected_draw(bright) > model.expected_draw(dim) + Power::new::<watt>(10.0));
    }
}
