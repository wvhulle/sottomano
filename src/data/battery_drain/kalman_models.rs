use std::time::Duration;

use adskalman::{ObservationModel, TransitionModelLinearNoControl};
use nalgebra::{Matrix1, Matrix1x3, Matrix3, Matrix3x1, U1, U3, Vector3};

pub(super) struct RandomWalk {
    identity: Matrix3<f64>,
    pub(super) drift_covariance: Matrix3<f64>,
}

impl RandomWalk {
    pub(super) fn over(elapsed: Duration, drift_variance_per_second: Vector3<f64>) -> Self {
        RandomWalk {
            identity: Matrix3::identity(),
            drift_covariance: Matrix3::from_diagonal(
                &(drift_variance_per_second * elapsed.as_secs_f64()),
            ),
        }
    }
}

impl TransitionModelLinearNoControl<f64, U3> for RandomWalk {
    fn F(&self) -> &Matrix3<f64> {
        &self.identity
    }
    fn FT(&self) -> &Matrix3<f64> {
        &self.identity
    }
    fn Q(&self) -> &Matrix3<f64> {
        &self.drift_covariance
    }
}

pub(super) struct LinearObservation {
    regressors: Matrix1x3<f64>,
    regressors_transposed: Matrix3x1<f64>,
    noise_variance: Matrix1<f64>,
}

impl LinearObservation {
    pub(super) fn new(regressors: Matrix1x3<f64>, noise_variance: f64) -> Self {
        LinearObservation {
            regressors,
            regressors_transposed: regressors.transpose(),
            noise_variance: Matrix1::new(noise_variance),
        }
    }
}

impl ObservationModel<f64, U3, U1> for LinearObservation {
    fn H(&self) -> &Matrix1x3<f64> {
        &self.regressors
    }
    fn HT(&self) -> &Matrix3x1<f64> {
        &self.regressors_transposed
    }
    fn R(&self) -> &Matrix1<f64> {
        &self.noise_variance
    }
}
