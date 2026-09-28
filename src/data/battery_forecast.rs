mod autocorrelated_residual;
mod charge_power_by_state_of_charge;
mod discharge_power_regression;
mod normally_distributed_power;
mod random_walk_kalman_models;
mod self_consistent_horizon;
mod time_until_empty;
mod time_until_full;

use std::time::Duration;

pub use autocorrelated_residual::ResidualProcess;
pub use charge_power_by_state_of_charge::{CHARGE_BINS, ChargeCurve, PowerBin, bin_of};
pub use discharge_power_regression::RegressionState;
pub use time_until_empty::time_until_empty;
pub use time_until_full::time_until_full;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub time: Duration,
    pub earliest: Duration,
    pub latest: Option<Duration>,
}

fn whole_minutes(seconds: f64) -> Option<Duration> {
    Duration::try_from_secs_f64((seconds / 60.0).round() * 60.0).ok()
}
