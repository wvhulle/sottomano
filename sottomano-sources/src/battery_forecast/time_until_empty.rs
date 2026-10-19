use std::{collections::HashMap, time::Duration};

use uom::si::f64::{Energy, Power};

use crate::{
    battery_forecast::{
        Estimate,
        discharge_power_regression::RegressionState,
        time_stepped_simulation::{Scenario, estimate, time_to_move},
    },
    battery_power_profiles::upower_battery_state::Battery,
    power_profile::{Conditions, Profile},
};

struct Draining<'a> {
    live: Option<Power>,
    active_profile: Profile,
    conditions: Conditions,
    profile_models: &'a HashMap<Profile, RegressionState>,
}

impl Draining<'_> {
    fn fit(&self, profile: Profile) -> Option<Power> {
        Some(
            self.profile_models
                .get(&profile)?
                .expected_draw(self.conditions),
        )
        .filter(|draw| *draw > Power::default())
    }

    fn learned_draw(
        &self,
        profile: Profile,
        scenario: Scenario,
        elapsed: Duration,
    ) -> Option<Power> {
        let fit = self.fit(profile)?;
        let deviation_now = self
            .live
            .filter(|_| profile == self.active_profile)
            .map_or_else(Power::default, |live| live - fit);
        Some(fit + scenario.deviation_after(deviation_now, elapsed))
    }

    fn active_draw(&self, scenario: Scenario, elapsed: Duration) -> Option<Power> {
        self.learned_draw(self.active_profile, scenario, elapsed)
            .or(self.live)
    }

    fn draw(&self, profile: Profile, scenario: Scenario, elapsed: Duration) -> Option<Power> {
        self.learned_draw(profile, scenario, elapsed).or_else(|| {
            let ratio = profile.scale() / self.active_profile.scale();
            Some(self.active_draw(scenario, elapsed)? * ratio)
        })
    }
}

#[must_use]
pub fn time_until_empty(
    battery: &Battery,
    active_profile: Profile,
    conditions: Conditions,
    profile_models: &HashMap<Profile, RegressionState>,
) -> HashMap<Profile, Option<Estimate>> {
    let draining = Draining {
        live: (battery.is_discharging() && battery.power > Power::default())
            .then_some(battery.power),
        active_profile,
        conditions,
        profile_models,
    };
    let has_energy = battery.energy > Energy::default();
    Profile::ALL
        .into_iter()
        .map(|profile| {
            let life = has_energy
                .then(|| {
                    estimate(|scenario| {
                        time_to_move(battery.energy, |elapsed, _| {
                            draining.draw(profile, scenario, elapsed)
                        })
                    })
                })
                .flatten();
            (profile, life)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::iter::repeat_n;

    use uom::si::{
        energy::watt_hour,
        f64::Ratio,
        power::watt,
        ratio::{percent, ratio},
    };

    use super::*;
    use crate::{
        battery_forecast::discharge_power_regression::trained,
        battery_power_profiles::upower_battery_state::{
            BATTERY_POLL_INTERVAL, ChargeBehaviour, ChargeStatus,
        },
        host_machine_identity::udev_hardware_identity::PowerSupplyIdentity,
    };

    fn on_battery(energy_wh: f64) -> Battery {
        Battery {
            status: Some(ChargeStatus::Discharging),
            capacity: Ratio::new::<percent>(50.0),
            energy: Energy::new::<watt_hour>(energy_wh),
            energy_full: Energy::new::<watt_hour>(60.0),
            power: Power::new::<watt>(0.0),
            charge_limit: Ratio::new::<ratio>(1.0),
            charge_behaviour: ChargeBehaviour::Auto,
            serial: String::new(),
            identity: PowerSupplyIdentity::default(),
        }
    }

    fn drawing(draw_w: f64) -> Battery {
        Battery {
            power: Power::new::<watt>(draw_w),
            ..on_battery(50.0)
        }
    }

    fn hrs(duration: Duration) -> f64 {
        duration.as_secs_f64() / 3600.0
    }

    fn balanced_life(
        battery: &Battery,
        conditions: Conditions,
        model: RegressionState,
    ) -> Estimate {
        time_until_empty(
            battery,
            Profile::Balanced,
            conditions,
            &HashMap::from([(Profile::Balanced, model)]),
        )[&Profile::Balanced]
            .expect("estimate present")
    }

    fn settled_at_ten_watts(conditions: Conditions) -> RegressionState {
        trained(conditions, repeat_n(Power::new::<watt>(10.0), 500))
    }

    #[test]
    fn brighter_screen_shortens_predicted_life() {
        let true_draw = |c: Conditions| Power::new::<watt>(5.0 + 20.0 * c.brightness_frac);
        let dim = Conditions::new(0.1, 0.0);
        let bright = Conditions::new(0.9, 0.0);
        let model = (0..80).fold(RegressionState::seed(true_draw(dim)), |model, _| {
            model
                .observe(dim, true_draw(dim), BATTERY_POLL_INTERVAL)
                .observe(bright, true_draw(bright), BATTERY_POLL_INTERVAL)
        });
        let life = |conditions| hrs(balanced_life(&on_battery(50.0), conditions, model).time);
        assert!(
            life(bright) < life(dim),
            "{} vs {}",
            life(bright),
            life(dim)
        );
    }

    #[test]
    fn heavier_draw_now_is_bounded_by_typical_and_persisting_draw() {
        let conditions = Conditions::new(0.5, 0.3);
        let life = balanced_life(&drawing(20.0), conditions, settled_at_ten_watts(conditions));
        let latest = life.latest.expect("bounded above");
        assert!(life.earliest < life.time && life.time < latest);
        assert!(
            (hrs(life.earliest) - 2.5).abs() < 0.02,
            "{}",
            hrs(life.earliest)
        );
        assert!((hrs(latest) - 5.0).abs() < 0.02, "{}", hrs(latest));
    }

    #[test]
    fn without_a_live_reading_the_scenarios_coincide() {
        let conditions = Conditions::new(0.5, 0.3);
        let life = balanced_life(
            &on_battery(50.0),
            conditions,
            settled_at_ten_watts(conditions),
        );
        assert_eq!(life.earliest, life.time);
        assert_eq!(life.latest, Some(life.time));
    }

    #[test]
    fn current_deviation_shifts_the_forecast() {
        let conditions = Conditions::new(0.5, 0.3);
        let model = settled_at_ten_watts(conditions);
        let life_while_drawing =
            |draw_w| hrs(balanced_life(&drawing(draw_w), conditions, model).time);
        assert!(life_while_drawing(12.0) < life_while_drawing(8.0));
    }

    #[test]
    fn cold_start_matches_seeded_draw() {
        let life = balanced_life(
            &on_battery(60.0),
            Conditions::new(0.7, 0.4),
            RegressionState::seed(Power::new::<watt>(12.0)),
        );
        assert!((hrs(life.time) - 5.0).abs() < 0.01);
    }

    #[test]
    fn scales_unobserved_profiles() {
        let estimates = time_until_empty(
            &on_battery(50.0),
            Profile::Balanced,
            Conditions::new(0.0, 0.0),
            &HashMap::from([(
                Profile::Balanced,
                RegressionState::seed(Power::new::<watt>(10.0)),
            )]),
        );
        let life = |profile| hrs(estimates[&profile].expect("estimate present").time);
        let saver = life(Profile::PowerSaver);
        let balanced = life(Profile::Balanced);
        let performance = life(Profile::Performance);
        assert!((performance - 50.0 / 15.0).abs() < 0.05, "{performance}");
        assert!((saver - 50.0 / 6.0).abs() < 0.05, "{saver}");
        assert!(performance < balanced && balanced < saver);
    }

    #[test]
    fn without_history_is_unknown() {
        let estimates = time_until_empty(
            &on_battery(50.0),
            Profile::Balanced,
            Conditions::new(0.5, 0.5),
            &HashMap::new(),
        );
        assert!(estimates[&Profile::Balanced].is_none());
    }
}
