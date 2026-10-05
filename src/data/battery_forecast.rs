mod charge_power_by_state_of_charge;
mod discharge_power_regression;
mod random_walk_kalman_models;
mod time_stepped_simulation;
mod time_until_empty;
mod time_until_full;

use std::time::Duration;

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
