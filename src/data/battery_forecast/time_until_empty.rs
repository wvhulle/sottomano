use std::{collections::HashMap, time::Duration};

use crate::data::{
    battery_forecast::{
        Estimate, discharge_power_regression::RegressionState,
        normally_distributed_power::NormalPower, self_consistent_horizon::self_consistent_forecast,
    },
    source::{
        Source,
        power_profile::{Conditions, Profile},
        sysfs_sensors::battery::Battery,
    },
};

pub fn time_until_empty(
    battery: &Battery,
    active_profile: Profile,
    conditions: Conditions,
    profile_models: &HashMap<Profile, RegressionState>,
) -> HashMap<Profile, Option<Estimate>> {
    let live_w = battery
        .power_w
        .filter(|w| battery.is_discharging() && *w > 0.0);
    let learned = |profile: Profile, horizon: Duration| {
        let model = profile_models.get(&profile)?;
        let current_w = live_w.filter(|_| profile == active_profile);
        Some(model.average_draw_over(horizon, conditions, current_w))
            .filter(|draw| draw.mean_w > 0.0)
    };
    let active_draw =
        |horizon| learned(active_profile, horizon).or_else(|| live_w.map(NormalPower::certain));
    Profile::ALL
        .into_iter()
        .map(|profile| {
            let ratio = profile.scale() / active_profile.scale();
            let draw = |horizon| {
                learned(profile, horizon)
                    .or_else(|| active_draw(horizon).map(|draw| draw.scaled(ratio)))
            };
            (profile, drain(battery.energy_j, draw))
        })
        .collect()
}

fn drain(
    energy_j: Option<f64>,
    average_draw_over: impl Fn(Duration) -> Option<NormalPower>,
) -> Option<Estimate> {
    let energy = energy_j.filter(|e| *e > 0.0)?;
    self_consistent_forecast(Battery::INTERVAL, average_draw_over, |draw| {
        Duration::try_from_secs_f64(energy / draw.mean_w).ok()
    })?
    .time_to_move(energy)
}

#[cfg(test)]
mod tests {
    use std::iter::repeat_n;

    use super::*;
    use crate::data::{
        battery_forecast::discharge_power_regression::{slow_wave, trained},
        source::sysfs_sensors::battery::ChargeStatus,
    };

    fn on_battery(energy_wh: f64) -> Battery {
        Battery {
            status: ChargeStatus::Discharging,
            capacity: 50,
            energy_j: Some(energy_wh * 3600.0),
            energy_full_j: None,
            power_w: None,
            limit_frac: 1.0,
            charge_behaviour: None,
            serial: String::new(),
        }
    }

    fn drawing(draw_w: f64) -> Battery {
        Battery {
            power_w: Some(draw_w),
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

    fn spread(estimate: Estimate) -> f64 {
        hrs(estimate.latest.expect("bounded above")) - hrs(estimate.earliest)
    }

    #[test]
    fn brighter_screen_shortens_predicted_life() {
        let true_draw = |c: Conditions| 5.0 + 20.0 * c.brightness_frac;
        let dim = Conditions::new(0.1, 0.0);
        let bright = Conditions::new(0.9, 0.0);
        let model = (0..80).fold(RegressionState::seed(true_draw(dim)), |model, _| {
            model
                .observe(dim, true_draw(dim))
                .observe(bright, true_draw(bright))
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
    fn interval_tightens_as_filter_learns() {
        let conditions = Conditions::new(0.5, 0.3);
        let battery = on_battery(50.0);
        let after_one = spread(balanced_life(
            &battery,
            conditions,
            trained(conditions, [10.0]),
        ));
        let after_many = spread(balanced_life(
            &battery,
            conditions,
            trained(conditions, repeat_n(10.0, 50)),
        ));
        assert!(after_many < after_one, "{after_many} vs {after_one}");
    }

    #[test]
    fn interval_is_skewed_towards_longer_life() {
        let conditions = Conditions::new(0.5, 0.3);
        let life = balanced_life(
            &on_battery(50.0),
            conditions,
            trained(conditions, slow_wave(3000)),
        );
        let below = hrs(life.time) - hrs(life.earliest);
        let above = hrs(life.latest.expect("bounded above")) - hrs(life.time);
        assert!(above > below, "{below} below vs {above} above");
    }

    #[test]
    fn current_deviation_persists_into_the_forecast() {
        let conditions = Conditions::new(0.5, 0.3);
        let model = trained(conditions, slow_wave(3000));
        let life_while_drawing =
            |draw_w| hrs(balanced_life(&drawing(draw_w), conditions, model).time);
        assert!(life_while_drawing(12.0) < life_while_drawing(8.0));
    }

    #[test]
    fn cold_start_matches_seeded_draw() {
        let life = balanced_life(
            &on_battery(60.0),
            Conditions::new(0.7, 0.4),
            RegressionState::seed(12.0),
        );
        assert!((hrs(life.time) - 5.0).abs() < 0.01);
    }

    #[test]
    fn scales_unobserved_profiles() {
        let estimates = time_until_empty(
            &on_battery(50.0),
            Profile::Balanced,
            Conditions::new(0.0, 0.0),
            &HashMap::from([(Profile::Balanced, RegressionState::seed(10.0))]),
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
