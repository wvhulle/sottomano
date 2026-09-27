mod kalman_models;
mod linear_regression;
mod normal_draw;
mod residual_process;

use std::{collections::HashMap, time::Duration};

pub use linear_regression::RegressionState;
use normal_draw::NormalDraw;
pub use residual_process::ResidualProcess;

use crate::data::source::{
    Source,
    power_profile::{PowerProfile, Profile},
};

const HORIZON_REFINEMENTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub time: Duration,
    pub earliest: Duration,
    pub latest: Option<Duration>,
}

pub fn estimate(
    reading: &PowerProfile,
    profile_models: &HashMap<Profile, RegressionState>,
) -> HashMap<Profile, Option<Estimate>> {
    let active_profile = reading.active_profile;
    let live_w = reading.draw_w.filter(|w| reading.discharging && *w > 0.0);
    let learned = |profile: Profile, horizon: Duration| {
        let model = profile_models.get(&profile)?;
        let current_w = live_w.filter(|_| profile == active_profile);
        Some(model.average_draw_over(horizon, reading.conditions, current_w))
            .filter(|draw| draw.mean_w > 0.0)
    };
    let active_draw =
        |horizon| learned(active_profile, horizon).or_else(|| live_w.map(NormalDraw::certain));
    Profile::ALL
        .into_iter()
        .map(|profile| {
            let ratio = profile.scale() / active_profile.scale();
            let draw = |horizon| {
                learned(profile, horizon)
                    .or_else(|| active_draw(horizon).map(|draw| draw.scaled(ratio)))
            };
            (profile, time_to_empty(reading.energy_j, draw))
        })
        .collect()
}

fn time_to_empty(
    energy_j: Option<f64>,
    average_draw_over: impl Fn(Duration) -> Option<NormalDraw>,
) -> Option<Estimate> {
    let energy = energy_j.filter(|e| *e > 0.0)?;
    let self_consistent_draw = (0..HORIZON_REFINEMENTS)
        .try_fold(average_draw_over(PowerProfile::INTERVAL)?, |draw, _| {
            average_draw_over(Duration::try_from_secs_f64(energy / draw.mean_w).ok()?)
        })?;
    self_consistent_draw.time_to_empty(energy)
}

#[cfg(test)]
mod tests {
    use std::iter::repeat_n;

    use super::{
        linear_regression::{slow_wave, trained},
        *,
    };
    use crate::data::source::power_profile::Conditions;

    impl PowerProfile {
        fn on_battery(active_profile: Profile, energy_wh: f64) -> Self {
            PowerProfile {
                active_profile,
                energy_j: Some(energy_wh * 3600.0),
                draw_w: None,
                discharging: true,
                battery_id: String::new(),
                conditions: Conditions::new(0.0, 0.0),
            }
        }

        fn at(mut self, conditions: Conditions) -> Self {
            self.conditions = conditions;
            self
        }

        fn drawing(mut self, draw_w: f64) -> Self {
            self.draw_w = Some(draw_w);
            self
        }
    }

    fn hrs(duration: Duration) -> f64 {
        duration.as_secs_f64() / 3600.0
    }

    fn balanced_life(reading: &PowerProfile, model: RegressionState) -> Estimate {
        estimate(reading, &HashMap::from([(Profile::Balanced, model)]))[&Profile::Balanced]
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
        let life = |conditions| {
            let reading = PowerProfile::on_battery(Profile::Balanced, 50.0).at(conditions);
            hrs(balanced_life(&reading, model).time)
        };
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
        let reading = PowerProfile::on_battery(Profile::Balanced, 50.0).at(conditions);
        let after_one = spread(balanced_life(&reading, trained(conditions, [10.0])));
        let after_many = spread(balanced_life(
            &reading,
            trained(conditions, repeat_n(10.0, 50)),
        ));
        assert!(after_many < after_one, "{after_many} vs {after_one}");
    }

    #[test]
    fn interval_is_skewed_towards_longer_life() {
        let conditions = Conditions::new(0.5, 0.3);
        let reading = PowerProfile::on_battery(Profile::Balanced, 50.0).at(conditions);
        let life = balanced_life(&reading, trained(conditions, slow_wave(3000)));
        let below = hrs(life.time) - hrs(life.earliest);
        let above = hrs(life.latest.expect("bounded above")) - hrs(life.time);
        assert!(above > below, "{below} below vs {above} above");
    }

    #[test]
    fn current_deviation_persists_into_the_forecast() {
        let conditions = Conditions::new(0.5, 0.3);
        let model = trained(conditions, slow_wave(3000));
        let life_while_drawing = |draw_w| {
            let reading = PowerProfile::on_battery(Profile::Balanced, 50.0)
                .at(conditions)
                .drawing(draw_w);
            hrs(balanced_life(&reading, model).time)
        };
        assert!(life_while_drawing(12.0) < life_while_drawing(8.0));
    }

    #[test]
    fn cold_start_matches_seeded_draw() {
        let reading =
            PowerProfile::on_battery(Profile::Balanced, 60.0).at(Conditions::new(0.7, 0.4));
        let life = balanced_life(&reading, RegressionState::seed(12.0));
        assert!((hrs(life.time) - 5.0).abs() < 0.01);
    }

    #[test]
    fn scales_unobserved_profiles() {
        let reading =
            PowerProfile::on_battery(Profile::Balanced, 50.0).at(Conditions::new(0.0, 0.0));
        let estimates = estimate(
            &reading,
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
        let reading =
            PowerProfile::on_battery(Profile::Balanced, 50.0).at(Conditions::new(0.5, 0.5));
        assert!(estimate(&reading, &HashMap::new())[&Profile::Balanced].is_none());
    }
}
