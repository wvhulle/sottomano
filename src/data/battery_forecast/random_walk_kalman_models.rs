use std::time::Duration;

use adskalman::{ObservationModel, TransitionModelLinearNoControl};
use nalgebra::{Const, Matrix1, SMatrix, SVector, U1};

pub(super) struct RandomWalk<const STATES: usize> {
    identity: SMatrix<f64, STATES, STATES>,
    pub(super) drift_covariance: SMatrix<f64, STATES, STATES>,
}

impl<const STATES: usize> RandomWalk<STATES> {
    pub(super) fn over(elapsed: Duration, drift_variance_per_second: SVector<f64, STATES>) -> Self {
        RandomWalk {
            identity: SMatrix::identity(),
            drift_covariance: SMatrix::from_diagonal(
                &(drift_variance_per_second * elapsed.as_secs_f64()),
            ),
        }
    }
}

impl<const STATES: usize> TransitionModelLinearNoControl<f64, Const<STATES>>
    for RandomWalk<STATES>
{
    fn F(&self) -> &SMatrix<f64, STATES, STATES> {
        &self.identity
    }
    fn FT(&self) -> &SMatrix<f64, STATES, STATES> {
        &self.identity
    }
    fn Q(&self) -> &SMatrix<f64, STATES, STATES> {
        &self.drift_covariance
    }
}

pub(super) struct LinearObservation<const STATES: usize> {
    regressors: SMatrix<f64, 1, STATES>,
    regressors_transposed: SMatrix<f64, STATES, 1>,
    noise_variance: Matrix1<f64>,
}

impl<const STATES: usize> LinearObservation<STATES> {
    pub(super) fn new(regressors: SMatrix<f64, 1, STATES>, noise_variance: f64) -> Self {
        LinearObservation {
            regressors,
            regressors_transposed: regressors.transpose(),
            noise_variance: Matrix1::new(noise_variance),
        }
    }
}

impl<const STATES: usize> ObservationModel<f64, Const<STATES>, U1> for LinearObservation<STATES> {
    fn H(&self) -> &SMatrix<f64, 1, STATES> {
        &self.regressors
    }
    fn HT(&self) -> &SMatrix<f64, STATES, 1> {
        &self.regressors_transposed
    }
    fn R(&self) -> &Matrix1<f64> {
        &self.noise_variance
    }
}
