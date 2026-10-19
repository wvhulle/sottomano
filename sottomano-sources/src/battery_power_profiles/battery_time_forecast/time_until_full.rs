use std::time::Duration;

use uom::si::f64::{Energy, Power};

use crate::battery_power_profiles::{
    battery_time_forecast::{
        Estimate,
        charge_power_by_state_of_charge::{ChargeCurve, LearnedCurve},
        time_stepped_simulation::{Scenario, estimate, time_to_move},
    },
    upower_battery_state::Battery,
};

struct Charging<'a> {
    battery: &'a Battery,
    curve: LearnedCurve,
    typical_now: Power,
    deviation_now: Power,
}

impl Charging<'_> {
    fn power_after(&self, scenario: Scenario, elapsed: Duration, filled: Energy) -> Power {
        let accepted = self
            .curve
            .power_at((self.battery.energy + filled) / self.battery.energy_full);
        let deviation = scenario.deviation_after(self.deviation_now, elapsed);
        if deviation < Power::default() {
            accepted.min(self.typical_now + deviation)
        } else {
            accepted + deviation
        }
    }
}

#[must_use]
pub fn time_until_full(battery: &Battery, curve: &ChargeCurve) -> Option<Estimate> {
    let to_fill = battery.energy_full * battery.charge_limit - battery.energy;
    if to_fill <= Energy::default() {
        return None;
    }
    let curve = curve.learned()?;
    let typical_now = curve.power_at(battery.energy / battery.energy_full);
    let charging = Charging {
        battery,
        deviation_now: if battery.is_charging() && battery.power > Power::default() {
            battery.power - typical_now
        } else {
            Power::default()
        },
        typical_now,
        curve,
    };
    estimate(|scenario| {
        time_to_move(to_fill, |elapsed, filled| {
            Some(charging.power_after(scenario, elapsed, filled))
        })
    })
}

#[cfg(test)]
mod tests {
    use uom::si::{energy::watt_hour, f64::Ratio, power::watt, ratio::ratio};

    use super::*;
    use crate::{
        battery_power_profiles::{
            battery_time_forecast::charge_power_by_state_of_charge::learned_curve,
            upower_battery_state::{ChargeBehaviour, ChargeStatus},
        },
        host_machine_identity::udev_hardware_identity::PowerSupplyIdentity,
    };

    const CAPACITY_WH: f64 = 50.0;

    fn charging_at(state_of_charge: f64) -> Battery {
        Battery {
            status: Some(ChargeStatus::Charging),
            capacity: Ratio::new::<ratio>(state_of_charge),
            energy: Energy::new::<watt_hour>(state_of_charge * CAPACITY_WH),
            energy_full: Energy::new::<watt_hour>(CAPACITY_WH),
            power: Power::new::<watt>(0.0),
            charge_limit: Ratio::new::<ratio>(1.0),
            charge_behaviour: ChargeBehaviour::Auto,
            serial: String::new(),
            identity: PowerSupplyIdentity::default(),
        }
    }

    fn drawing(power_w: f64, battery: Battery) -> Battery {
        Battery {
            power: Power::new::<watt>(power_w),
            ..battery
        }
    }

    fn minutes(duration: Duration) -> f64 {
        duration.as_secs_f64() / 60.0
    }

    fn constant(power_w: f64) -> impl Fn(Ratio) -> Power {
        move |_| Power::new::<watt>(power_w)
    }

    fn tapered(soc: Ratio) -> Power {
        if soc < Ratio::new::<ratio>(0.8) {
            Power::new::<watt>(30.0)
        } else {
            Power::new::<watt>(10.0)
        }
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
        let battery = charging_at(0.2);
        assert!(
            median_minutes(&battery, &learned_curve(tapered))
                > median_minutes(&battery, &learned_curve(constant(30.0)))
        );
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
        let at_limit = Battery {
            charge_limit: Ratio::new::<ratio>(0.8),
            ..charging_at(0.85)
        };
        assert!(time_until_full(&at_limit, &learned_curve(constant(30.0))).is_none());
    }

    #[test]
    fn without_a_curve_there_is_no_estimate() {
        assert!(time_until_full(&charging_at(0.3), &ChargeCurve::default()).is_none());
    }

    #[test]
    fn typical_power_collapses_the_scenarios() {
        let estimate = time_until_full(&drawing(30.0, charging_at(0.3)), &learned_curve(tapered))
            .expect("estimate");
        assert_eq!(estimate.earliest, estimate.time);
        assert_eq!(estimate.latest, Some(estimate.time));
    }

    #[test]
    fn heavy_load_is_bounded_by_typical_and_persisting_supply() {
        let curve = learned_curve(|soc| {
            if soc < Ratio::new::<ratio>(0.95) {
                Power::new::<watt>(40.0)
            } else {
                Power::new::<watt>(6.5)
            }
        });
        let estimate = time_until_full(&drawing(7.8, charging_at(0.16)), &curve).expect("estimate");
        let latest = estimate.latest.expect("bounded above");
        assert!(estimate.earliest < estimate.time && estimate.time < latest);
        let persisting_minutes = (0.79 * CAPACITY_WH / 7.8 + 0.05 * CAPACITY_WH / 6.5) * 60.0;
        assert!(
            (minutes(latest) - persisting_minutes).abs() <= 1.0,
            "{} vs {persisting_minutes}",
            minutes(latest)
        );
    }

    #[test]
    fn light_load_finishes_sooner_than_typical() {
        let curve = learned_curve(constant(30.0));
        let estimate = time_until_full(&drawing(40.0, charging_at(0.5)), &curve).expect("estimate");
        assert!(estimate.earliest < estimate.time);
        assert_eq!(estimate.latest, Some(Duration::from_mins(50)));
    }
}
