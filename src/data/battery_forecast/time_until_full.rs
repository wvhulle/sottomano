use std::{ops::Range, time::Duration};

use crate::data::{
    battery_forecast::{
        Estimate,
        charge_power_by_state_of_charge::{ChargeCurve, bin_lower_edge, bin_of},
        normally_distributed_power::NormalPower,
        self_consistent_horizon::self_consistent_forecast,
        whole_minutes,
    },
    source::{Source, sysfs_sensors::battery::Battery},
};

struct RemainingBin {
    energy_j: f64,
    power: NormalPower,
}

struct ChargeTarget {
    capacity_j: f64,
    energy_to_fill: Range<f64>,
}

impl ChargeTarget {
    fn of(battery: &Battery) -> Option<Self> {
        let capacity_j = battery.energy_full_j.filter(|capacity| *capacity > 0.0)?;
        let target_j = capacity_j * battery.limit_frac;
        Some(ChargeTarget {
            capacity_j,
            energy_to_fill: battery.energy_j.filter(|now| *now < target_j)?..target_j,
        })
    }

    fn energy_left_in(&self, bin: usize) -> f64 {
        let lower_j = bin_lower_edge(bin) * self.capacity_j;
        let upper_j = bin_lower_edge(bin + 1) * self.capacity_j;
        upper_j.min(self.energy_to_fill.end) - lower_j.max(self.energy_to_fill.start)
    }

    fn remaining_bins(&self, curve: &ChargeCurve) -> Option<Vec<RemainingBin>> {
        let first_bin = bin_of(self.energy_to_fill.start / self.capacity_j);
        let last_bin = bin_of(self.energy_to_fill.end / self.capacity_j);
        let bins = (first_bin..=last_bin)
            .filter(|bin| self.energy_left_in(*bin) > 0.0)
            .map(|bin| {
                Some(RemainingBin {
                    energy_j: self.energy_left_in(bin),
                    power: curve.power_in(bin)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(bins).filter(|bins| !bins.is_empty())
    }
}

fn fill_seconds(
    remaining: &[RemainingBin],
    deviation: NormalPower,
    standard_deviations: f64,
) -> Option<f64> {
    remaining
        .iter()
        .map(|bin| {
            let power_w = bin
                .power
                .plus(deviation)
                .shifted_by_deviations(standard_deviations);
            (power_w > 0.0).then_some(bin.energy_j / power_w)
        })
        .sum()
}

fn samples_within(horizon: Duration) -> f64 {
    (horizon.as_secs_f64() / Battery::INTERVAL.as_secs_f64()).max(1.0)
}

pub fn time_until_full(battery: &Battery, curve: &ChargeCurve) -> Option<Estimate> {
    let remaining = ChargeTarget::of(battery)?.remaining_bins(curve)?;
    let current_deviation = battery
        .power_w
        .filter(|_| battery.is_charging())
        .map(|power_w| power_w - remaining[0].power.mean_w);
    let deviation = self_consistent_forecast(
        Battery::INTERVAL,
        |horizon| {
            Some(
                curve
                    .residuals
                    .average_over(samples_within(horizon), current_deviation),
            )
        },
        |deviation| Duration::try_from_secs_f64(fill_seconds(&remaining, *deviation, 0.0)?).ok(),
    )?;
    Some(Estimate {
        time: whole_minutes(fill_seconds(&remaining, deviation, 0.0)?)?,
        earliest: whole_minutes(fill_seconds(&remaining, deviation, 1.0)?)?,
        latest: fill_seconds(&remaining, deviation, -1.0).and_then(whole_minutes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        battery_forecast::{ResidualProcess, charge_power_by_state_of_charge::learned_curve},
        source::sysfs_sensors::battery::ChargeStatus,
    };

    #[test]
    fn momentary_deviation_without_autocorrelation_is_ignored() {
        let curve = learned_curve(constant(30.0));
        let at_power = |power_w| Battery {
            power_w: Some(power_w),
            ..charging_at(0.5)
        };
        assert!(
            (median_minutes(&at_power(40.0), &curve) - median_minutes(&at_power(20.0), &curve))
                .abs()
                <= 1.0
        );
    }

    const CAPACITY_WH: f64 = 50.0;

    fn charging_at(state_of_charge: f64) -> Battery {
        Battery {
            status: ChargeStatus::Charging,
            capacity: (state_of_charge * 100.0) as u8,
            energy_j: Some(state_of_charge * CAPACITY_WH * 3600.0),
            energy_full_j: Some(CAPACITY_WH * 3600.0),
            power_w: None,
            limit_frac: 1.0,
            charge_behaviour: None,
            serial: String::new(),
        }
    }

    fn minutes(duration: Duration) -> f64 {
        duration.as_secs_f64() / 60.0
    }

    fn constant(power_w: f64) -> impl Fn(f64) -> f64 {
        move |_| power_w
    }

    fn median_minutes(battery: &Battery, curve: &ChargeCurve) -> f64 {
        minutes(time_until_full(battery, curve).expect("estimate").time)
    }

    #[test]
    fn constant_power_takes_energy_over_power() {
        let curve = learned_curve(constant(30.0));
        let expected = 0.8 * CAPACITY_WH / 30.0 * 60.0;
        let median = median_minutes(&charging_at(0.2), &curve);
        assert!((median - expected).abs() <= 1.0, "{median} vs {expected}");
    }

    #[test]
    fn taper_above_eighty_percent_takes_longer() {
        let tapered = learned_curve(|soc| if soc < 0.8 { 30.0 } else { 10.0 });
        let flat = learned_curve(constant(30.0));
        let battery = charging_at(0.2);
        assert!(median_minutes(&battery, &tapered) > median_minutes(&battery, &flat));
    }

    #[test]
    fn lower_charge_limit_finishes_sooner() {
        let curve = learned_curve(constant(30.0));
        let limited = Battery {
            limit_frac: 0.8,
            ..charging_at(0.2)
        };
        assert!(median_minutes(&limited, &curve) < median_minutes(&charging_at(0.2), &curve));
    }

    #[test]
    fn already_at_the_limit_has_no_estimate() {
        let curve = learned_curve(constant(30.0));
        let at_limit = Battery {
            limit_frac: 0.8,
            ..charging_at(0.85)
        };
        assert!(time_until_full(&at_limit, &curve).is_none());
    }

    #[test]
    fn persistent_deviation_right_now_shifts_the_forecast() {
        let curve = ChargeCurve {
            residuals: ResidualProcess {
                variance: 4.0,
                lag_covariance: 3.9,
                previous: None,
            },
            ..learned_curve(constant(30.0))
        };
        let at_power = |power_w| Battery {
            power_w: Some(power_w),
            ..charging_at(0.5)
        };
        assert!(median_minutes(&at_power(40.0), &curve) < median_minutes(&at_power(20.0), &curve));
    }

    #[test]
    fn interval_brackets_the_median() {
        let curve = learned_curve(constant(30.0));
        let estimate = time_until_full(&charging_at(0.3), &curve).expect("estimate");
        assert!(estimate.earliest <= estimate.time);
        assert!(
            estimate
                .latest
                .is_some_and(|latest| latest >= estimate.time)
        );
    }

    #[test]
    fn borrowed_bins_widen_the_interval() {
        let learned_everywhere = learned_curve(constant(30.0));
        let learned_once = ChargeCurve::default()
            .observe(0.32, 30.0)
            .observe(0.32, 30.0);
        let width = |curve: &ChargeCurve| {
            let estimate = time_until_full(&charging_at(0.3), curve).expect("estimate");
            estimate.latest.map_or(f64::INFINITY, |latest| {
                minutes(latest) - minutes(estimate.earliest)
            })
        };
        assert!(width(&learned_once) > width(&learned_everywhere));
    }

    #[test]
    fn without_a_curve_there_is_no_estimate() {
        assert!(time_until_full(&charging_at(0.3), &ChargeCurve::default()).is_none());
    }
}
