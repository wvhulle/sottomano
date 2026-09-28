use std::{ops::Range, time::Duration};

use uom::si::{
    f64::{Energy, Power, Time},
    power::watt,
    time::second,
};

use crate::data::{
    battery_forecast::{
        Estimate,
        charge_power_by_state_of_charge::{ChargeCurve, bin_lower_edge, bin_of},
        duration_of,
        normally_distributed_power::NormalPower,
        self_consistent_horizon::self_consistent_forecast,
        whole_minutes,
    },
    source::{Source, sysfs_sensors::battery::Battery},
};

struct RemainingBin {
    energy: Energy,
    power: NormalPower,
}

struct ChargeTarget {
    capacity: Energy,
    energy_to_fill: Range<Energy>,
}

impl ChargeTarget {
    fn of(battery: &Battery) -> Option<Self> {
        let target = battery.energy_full * battery.charge_limit;
        (battery.energy < target).then_some(ChargeTarget {
            capacity: battery.energy_full,
            energy_to_fill: battery.energy..target,
        })
    }

    fn energy_left_in(&self, bin: usize) -> Energy {
        let lower = self.capacity * bin_lower_edge(bin);
        let upper = self.capacity * bin_lower_edge(bin + 1);
        upper.min(self.energy_to_fill.end) - lower.max(self.energy_to_fill.start)
    }

    fn remaining_bins(&self, curve: &ChargeCurve) -> Option<Vec<RemainingBin>> {
        let first_bin = bin_of(self.energy_to_fill.start / self.capacity);
        let last_bin = bin_of(self.energy_to_fill.end / self.capacity);
        let nothing = Energy::default();
        let bins = (first_bin..=last_bin)
            .filter(|bin| self.energy_left_in(*bin) > nothing)
            .map(|bin| {
                Some(RemainingBin {
                    energy: self.energy_left_in(bin),
                    power: curve.power_in(bin)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(bins).filter(|bins| !bins.is_empty())
    }
}

fn fill_time(
    remaining: &[RemainingBin],
    deviation: NormalPower,
    standard_deviations: f64,
) -> Option<Time> {
    remaining
        .iter()
        .try_fold(Time::new::<second>(0.0), |total, bin| {
            let power = bin
                .power
                .plus(deviation)
                .shifted_by_deviations(standard_deviations);
            (power > Power::new::<watt>(0.0)).then(|| total + bin.energy / power)
        })
}

fn samples_within(horizon: Duration) -> f64 {
    (horizon.as_secs_f64() / Battery::INTERVAL.as_secs_f64()).max(1.0)
}

pub fn time_until_full(battery: &Battery, curve: &ChargeCurve) -> Option<Estimate> {
    let remaining = ChargeTarget::of(battery)?.remaining_bins(curve)?;
    let current_deviation = (battery.is_charging() && battery.power > Power::new::<watt>(0.0))
        .then(|| battery.power - remaining[0].power.mean);
    let deviation = self_consistent_forecast(
        Battery::INTERVAL,
        |horizon| {
            Some(
                curve
                    .residuals
                    .average_over(samples_within(horizon), current_deviation),
            )
        },
        |deviation| duration_of(fill_time(&remaining, *deviation, 0.0)?),
    )?;
    Some(Estimate {
        time: whole_minutes(fill_time(&remaining, deviation, 0.0)?)?,
        earliest: whole_minutes(fill_time(&remaining, deviation, 1.0)?)?,
        latest: fill_time(&remaining, deviation, -1.0).and_then(whole_minutes),
    })
}

#[cfg(test)]
mod tests {
    use uom::si::{energy::watt_hour, f64::Ratio, ratio::ratio};

    use super::*;
    use crate::data::{
        battery_forecast::{ResidualProcess, charge_power_by_state_of_charge::learned_curve},
        source::sysfs_sensors::battery::{ChargeBehaviour, ChargeStatus},
    };

    #[test]
    fn momentary_deviation_without_autocorrelation_is_ignored() {
        let curve = learned_curve(constant(30.0));
        let at_power = |power_w| Battery {
            power: Power::new::<watt>(power_w),
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
            capacity: Ratio::new::<ratio>(state_of_charge),
            energy: Energy::new::<watt_hour>(state_of_charge * CAPACITY_WH),
            energy_full: Energy::new::<watt_hour>(CAPACITY_WH),
            power: Power::new::<watt>(0.0),
            charge_limit: Ratio::new::<ratio>(1.0),
            charge_behaviour: ChargeBehaviour::Auto,
            charger: None,
            serial: String::new(),
        }
    }

    fn minutes(duration: Duration) -> f64 {
        duration.as_secs_f64() / 60.0
    }

    fn constant(power_w: f64) -> impl Fn(Ratio) -> Power {
        move |_| Power::new::<watt>(power_w)
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
        let tapered = learned_curve(|soc| {
            if soc < Ratio::new::<ratio>(0.8) {
                Power::new::<watt>(30.0)
            } else {
                Power::new::<watt>(10.0)
            }
        });
        let flat = learned_curve(constant(30.0));
        let battery = charging_at(0.2);
        assert!(median_minutes(&battery, &tapered) > median_minutes(&battery, &flat));
    }

    #[test]
    fn lower_charge_limit_finishes_sooner() {
        let curve = learned_curve(constant(30.0));
        let limited = Battery {
            charge_limit: Ratio::new::<ratio>(0.8),
            ..charging_at(0.2)
        };
        assert!(median_minutes(&limited, &curve) < median_minutes(&charging_at(0.2), &curve));
    }

    #[test]
    fn already_at_the_limit_has_no_estimate() {
        let curve = learned_curve(constant(30.0));
        let at_limit = Battery {
            charge_limit: Ratio::new::<ratio>(0.8),
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
            power: Power::new::<watt>(power_w),
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
        let bin_at_32_percent = Ratio::new::<ratio>(0.32);
        let learned_once = ChargeCurve::default()
            .observe(bin_at_32_percent, Power::new::<watt>(30.0))
            .observe(bin_at_32_percent, Power::new::<watt>(30.0));
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
