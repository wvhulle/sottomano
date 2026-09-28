use crate::data::battery_forecast::{Estimate, whole_minutes};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NormalPower {
    pub(super) mean_w: f64,
    pub(super) variance: f64,
}

impl NormalPower {
    pub(super) fn certain(mean_w: f64) -> Self {
        NormalPower {
            mean_w,
            variance: 0.0,
        }
    }

    pub(super) fn plus(self, other: NormalPower) -> Self {
        NormalPower {
            mean_w: self.mean_w + other.mean_w,
            variance: self.variance + other.variance,
        }
    }

    pub(super) fn scaled(self, ratio: f64) -> Self {
        NormalPower {
            mean_w: self.mean_w * ratio,
            variance: self.variance * ratio * ratio,
        }
    }

    pub(super) fn standard_deviation(&self) -> f64 {
        self.variance.sqrt()
    }

    pub(super) fn shifted_by_deviations(&self, standard_deviations: f64) -> f64 {
        self.mean_w + standard_deviations * self.standard_deviation()
    }

    pub(super) fn time_to_move(&self, energy_j: f64) -> Option<Estimate> {
        let seconds_at = |power_w: f64| (power_w > 0.0).then_some(energy_j / power_w);
        Some(Estimate {
            time: whole_minutes(seconds_at(self.mean_w)?)?,
            earliest: whole_minutes(seconds_at(self.shifted_by_deviations(1.0))?)?,
            latest: seconds_at(self.shifted_by_deviations(-1.0)).and_then(whole_minutes),
        })
    }
}
