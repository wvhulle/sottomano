use uom::si::{
    f64::{Energy, Power},
    power::watt,
};

use crate::data::battery_forecast::{Estimate, whole_minutes};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NormalPower {
    pub(super) mean: Power,
    pub(super) spread: Power,
}

impl NormalPower {
    pub(super) fn from_watts(mean_w: f64, variance_w2: f64) -> Self {
        NormalPower {
            mean: Power::new::<watt>(mean_w),
            spread: Power::new::<watt>(variance_w2.sqrt()),
        }
    }

    pub(super) fn certain(mean: Power) -> Self {
        NormalPower {
            mean,
            spread: Power::new::<watt>(0.0),
        }
    }

    pub(super) fn plus(self, other: NormalPower) -> Self {
        NormalPower {
            mean: self.mean + other.mean,
            spread: (self.spread * self.spread + other.spread * other.spread).sqrt(),
        }
    }

    pub(super) fn scaled(self, ratio: f64) -> Self {
        NormalPower {
            mean: self.mean * ratio,
            spread: self.spread * ratio.abs(),
        }
    }

    pub(super) fn shifted_by_deviations(&self, standard_deviations: f64) -> Power {
        self.mean + self.spread * standard_deviations
    }

    pub(super) fn time_to_move(&self, energy: Energy) -> Option<Estimate> {
        let time_at = |power: Power| (power > Power::new::<watt>(0.0)).then(|| energy / power);
        Some(Estimate {
            time: whole_minutes(time_at(self.mean)?)?,
            earliest: whole_minutes(time_at(self.shifted_by_deviations(1.0))?)?,
            latest: time_at(self.shifted_by_deviations(-1.0)).and_then(whole_minutes),
        })
    }
}
