use uom::si::f64::Power;

use crate::data::battery_forecast::normally_distributed_power::NormalPower;

const SEED_VARIANCE: f64 = 4.0;
const MIN_VARIANCE: f64 = 0.01;
const LEARNING_RATE: f64 = 0.002;
const MAX_AUTOCORRELATION: f64 = 0.999;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResidualProcess {
    pub variance: f64,
    pub lag_covariance: f64,
    pub previous: Option<f64>,
}

impl ResidualProcess {
    pub(super) fn seed() -> Self {
        ResidualProcess {
            variance: SEED_VARIANCE,
            lag_covariance: 0.0,
            previous: None,
        }
    }

    pub fn autocorrelation(&self) -> f64 {
        (self.lag_covariance / self.variance).clamp(0.0, MAX_AUTOCORRELATION)
    }

    fn innovation_variance(&self) -> f64 {
        let rho = self.autocorrelation();
        self.variance * (1.0 - rho * rho)
    }

    pub(super) fn variance_per_independent_sample(&self) -> f64 {
        let rho = self.autocorrelation();
        self.variance * (1.0 + rho) / (1.0 - rho)
    }

    pub(super) fn absorb(self, innovation: f64, fit_variance: f64) -> Self {
        let chained = ResidualProcess {
            previous: Some(innovation),
            ..self
        };
        if fit_variance >= self.variance {
            return chained;
        }
        let blend = |learned: f64, sample: f64| learned + LEARNING_RATE * (sample - learned);
        ResidualProcess {
            variance: blend(self.variance, innovation * innovation - fit_variance)
                .max(MIN_VARIANCE),
            lag_covariance: self.previous.map_or(self.lag_covariance, |previous| {
                blend(self.lag_covariance, previous * innovation)
            }),
            ..chained
        }
    }

    pub(super) fn interrupted(self) -> Self {
        ResidualProcess {
            previous: None,
            ..self
        }
    }

    pub(super) fn average_over(&self, steps: f64, current: Option<Power>) -> NormalPower {
        match current {
            Some(residual) => self.average_given_current(steps, residual),
            None => self.average_unconditionally(steps),
        }
    }

    fn average_given_current(&self, steps: f64, residual: Power) -> NormalPower {
        NormalPower {
            mean: residual * self.persistence(steps),
            ..NormalPower::from_watts(0.0, self.unforeseen_variance(steps))
        }
    }

    fn average_unconditionally(&self, steps: f64) -> NormalPower {
        let persistence = self.persistence(steps);
        NormalPower::from_watts(
            0.0,
            self.unforeseen_variance(steps) + persistence * persistence * self.variance,
        )
    }

    fn persistence(&self, steps: f64) -> f64 {
        geometric_sum(self.autocorrelation(), steps) / steps
    }

    fn unforeseen_variance(&self, steps: f64) -> f64 {
        let rho = self.autocorrelation();
        let squared_unrealised_influence =
            steps - 2.0 * geometric_sum(rho, steps) + geometric_sum(rho * rho, steps);
        self.innovation_variance() * squared_unrealised_influence
            / ((1.0 - rho).powi(2) * steps * steps)
    }
}

fn geometric_sum(ratio: f64, terms: f64) -> f64 {
    ratio * (1.0 - ratio.powf(terms)) / (1.0 - ratio)
}

#[cfg(test)]
mod tests {
    use uom::si::power::watt;

    use super::*;

    fn white_noise(variance: f64) -> ResidualProcess {
        ResidualProcess {
            variance,
            lag_covariance: 0.0,
            previous: None,
        }
    }

    #[test]
    fn white_noise_average_has_variance_over_n() {
        let average = white_noise(4.0).average_over(100.0, Some(Power::new::<watt>(3.0)));
        assert!(average.mean.get::<watt>().abs() < 1e-12);
        assert!((average.spread.get::<watt>() - 0.2).abs() < 1e-12);
    }

    #[test]
    fn geometric_sum_matches_explicit_series() {
        let explicit: f64 = (1..=5).map(|k| 0.5_f64.powi(k)).sum();
        assert!((geometric_sum(0.5, 5.0) - explicit).abs() < 1e-12);
    }

    #[test]
    fn correlated_residuals_average_out_slower_than_white_noise() {
        let correlated = ResidualProcess {
            variance: 4.0,
            lag_covariance: 3.6,
            previous: None,
        };
        assert!(
            correlated.average_over(100.0, None).spread
                > white_noise(4.0).average_over(100.0, None).spread
        );
    }
}
