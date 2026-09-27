use std::time::Duration;

use crate::data::battery_drain::Estimate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NormalDraw {
    pub(super) mean_w: f64,
    pub(super) variance: f64,
}

impl NormalDraw {
    pub(super) fn certain(mean_w: f64) -> Self {
        NormalDraw {
            mean_w,
            variance: 0.0,
        }
    }

    pub(super) fn plus(self, other: NormalDraw) -> Self {
        NormalDraw {
            mean_w: self.mean_w + other.mean_w,
            variance: self.variance + other.variance,
        }
    }

    pub(super) fn scaled(self, ratio: f64) -> Self {
        NormalDraw {
            mean_w: self.mean_w * ratio,
            variance: self.variance * ratio * ratio,
        }
    }

    fn standard_deviation(&self) -> f64 {
        self.variance.sqrt()
    }

    pub(super) fn time_to_empty(&self, energy_j: f64) -> Option<Estimate> {
        let one_sigma_heavier = self.mean_w + self.standard_deviation();
        let one_sigma_lighter = self.mean_w - self.standard_deviation();
        Some(Estimate {
            time: drain_duration(energy_j, self.mean_w)?,
            earliest: drain_duration(energy_j, one_sigma_heavier)?,
            latest: Some(one_sigma_lighter)
                .filter(|draw_w| *draw_w > 0.0)
                .and_then(|draw_w| drain_duration(energy_j, draw_w)),
        })
    }
}

fn drain_duration(energy_j: f64, draw_w: f64) -> Option<Duration> {
    let seconds = energy_j / draw_w;
    Duration::try_from_secs_f64((seconds / 60.0).round() * 60.0).ok()
}
