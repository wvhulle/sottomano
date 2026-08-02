use std::{collections::HashMap, time::Duration};

use adskalman::{
    KalmanFilterNoControl, ObservationModel, StateAndCovariance, TransitionModelLinearNoControl,
};
use nalgebra::{Matrix1, Matrix1x3, Matrix3, Matrix3x1, U1, U3, Vector1, Vector3};
use zbus::{blocking::Connection, proxy};

use crate::data::source::{
    BarWidget, Glyph, Source, TooltipBody,
    font_awesome::{FontAwesome, Solid},
    sysfs_sensors,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
    PowerSaver,
    Balanced,
    Performance,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::PowerSaver, Profile::Balanced, Profile::Performance];

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "power-saver" => Some(Profile::PowerSaver),
            "balanced" => Some(Profile::Balanced),
            "performance" => Some(Profile::Performance),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Profile::PowerSaver => "power-saver",
            Profile::Balanced => "balanced",
            Profile::Performance => "performance",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Profile::PowerSaver => "Power saver",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }

    pub fn icon(self) -> FontAwesome {
        match self {
            Profile::PowerSaver => Solid::Leaf.into(),
            Profile::Balanced => Solid::ScaleBalanced.into(),
            Profile::Performance => Solid::Rocket.into(),
        }
    }

    pub fn scale(self) -> f64 {
        match self {
            Profile::PowerSaver => 0.6,
            Profile::Balanced => 1.0,
            Profile::Performance => 1.5,
        }
    }

    pub fn position(self) -> f32 {
        match self {
            Profile::PowerSaver => 0.0,
            Profile::Balanced => 50.0,
            Profile::Performance => 100.0,
        }
    }

    pub fn from_position(value: f32) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        match (value / 50.0).round() as i32 {
            i if i <= 0 => Profile::PowerSaver,
            1 => Profile::Balanced,
            _ => Profile::Performance,
        }
    }
}

#[proxy(
    interface = "net.hadess.PowerProfiles",
    default_service = "net.hadess.PowerProfiles",
    default_path = "/net/hadess/PowerProfiles"
)]
trait PowerProfiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, value: &str) -> zbus::Result<()>;
}

fn daemon() -> Option<PowerProfilesProxyBlocking<'static>> {
    PowerProfilesProxyBlocking::new(&Connection::system().ok()?).ok()
}

pub fn active() -> Option<Profile> {
    Profile::from_name(&daemon()?.active_profile().ok()?)
}

pub fn set(profile: Profile) -> bool {
    daemon().is_some_and(|daemon| daemon.set_active_profile(profile.name()).is_ok())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    pub brightness_frac: f64,
    pub cpu_busy_frac: f64,
}

impl Conditions {
    pub fn new(brightness_frac: f64, cpu_busy_frac: f64) -> Self {
        Conditions {
            brightness_frac,
            cpu_busy_frac,
        }
    }

    pub fn current() -> Self {
        Conditions::new(
            sysfs_sensors::internal_brightness_frac().unwrap_or(0.0),
            sysfs_sensors::cpu_usage::cpu_busy_frac().unwrap_or(0.0),
        )
    }

    fn design_row(self) -> Matrix1x3<f64> {
        Matrix1x3::new(1.0, self.brightness_frac, self.cpu_busy_frac)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerProfile {
    pub active_profile: Profile,
    pub energy_j: Option<f64>,
    pub draw_w: Option<f64>,
    pub discharging: bool,
    pub battery_id: String,
    pub conditions: Conditions,
}

impl BarWidget for PowerProfile {
    fn icon(&self) -> Glyph {
        self.active_profile.icon().into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

impl Source for PowerProfile {
    const NAME: &'static str = "power-profile";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn poll() -> Option<Self> {
        let active_profile = active()?;
        let energy = sysfs_sensors::battery_energy();
        Some(PowerProfile {
            active_profile,
            energy_j: energy.as_ref().and_then(|e| e.energy_j),
            draw_w: energy.as_ref().and_then(|e| e.draw_w),
            discharging: energy.as_ref().is_some_and(|e| e.discharging),
            battery_id: energy.map(|e| e.serial).unwrap_or_default(),
            conditions: Conditions::current(),
        })
    }
}

const Q_BASE: f64 = 0.25;
const Q_COEFF: f64 = 0.05;
const MEASUREMENT_NOISE: f64 = 4.0;
const SEED_VARIANCE_BASE: f64 = 100.0;
const SEED_VARIANCE_COEFF: f64 = 400.0;

struct RandomWalk {
    f: Matrix3<f64>,
    q: Matrix3<f64>,
}

impl TransitionModelLinearNoControl<f64, U3> for RandomWalk {
    fn F(&self) -> &Matrix3<f64> {
        &self.f
    }
    fn FT(&self) -> &Matrix3<f64> {
        &self.f
    }
    fn Q(&self) -> &Matrix3<f64> {
        &self.q
    }
}

struct LinearObservation {
    h: Matrix1x3<f64>,
    ht: Matrix3x1<f64>,
    r: Matrix1<f64>,
}

impl ObservationModel<f64, U3, U1> for LinearObservation {
    fn H(&self) -> &Matrix1x3<f64> {
        &self.h
    }
    fn HT(&self) -> &Matrix3x1<f64> {
        &self.ht
    }
    fn R(&self) -> &Matrix1<f64> {
        &self.r
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionState {
    pub coefficients: Vector3<f64>,
    pub covariance: Matrix3<f64>,
}

impl RegressionState {
    pub fn seed(first_draw_w: f64) -> Self {
        RegressionState {
            coefficients: Vector3::new(first_draw_w, 0.0, 0.0),
            covariance: Matrix3::from_diagonal(&Vector3::new(
                SEED_VARIANCE_BASE,
                SEED_VARIANCE_COEFF,
                SEED_VARIANCE_COEFF,
            )),
        }
    }

    pub fn observe(self, conditions: Conditions, observed_w: f64) -> Self {
        let transition = RandomWalk {
            f: Matrix3::identity(),
            q: Matrix3::from_diagonal(&Vector3::new(Q_BASE, Q_COEFF, Q_COEFF)),
        };
        let regressor_row = conditions.design_row();
        let observation = LinearObservation {
            h: regressor_row,
            ht: regressor_row.transpose(),
            r: Matrix1::new(MEASUREMENT_NOISE),
        };
        let filter = KalmanFilterNoControl::new(&transition, &observation);
        let prior = StateAndCovariance::new(self.coefficients, self.covariance);
        filter
            .step(&prior, &Vector1::new(observed_w))
            .map_or(self, |posterior| RegressionState {
                coefficients: *posterior.state(),
                covariance: *posterior.covariance(),
            })
    }

    fn predict(&self, conditions: Conditions) -> Prediction {
        let regressor_row = conditions.design_row();
        Prediction {
            draw_w: (regressor_row * self.coefficients)[0],
            draw_variance: (regressor_row * self.covariance * regressor_row.transpose())[(0, 0)],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Prediction {
    draw_w: f64,
    draw_variance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    pub time: Duration,
    pub band: Duration,
}

pub fn estimate(
    reading: &PowerProfile,
    profile_models: &HashMap<Profile, RegressionState>,
) -> HashMap<Profile, Option<Estimate>> {
    let active_profile = reading.active_profile;
    let live_w = reading.draw_w.filter(|_| reading.discharging);
    let learned = |profile| {
        profile_models
            .get(&profile)
            .map(|model| model.predict(reading.conditions))
    };
    let active_draw = learned(active_profile)
        .filter(|prediction| prediction.draw_w > 0.0)
        .or_else(|| live_w.filter(|w| *w > 0.0).map(observed_draw));
    let scaled_from_active = |profile: Profile| {
        let ratio = profile.scale() / active_profile.scale();
        active_draw.map(|prediction| Prediction {
            draw_w: prediction.draw_w * ratio,
            draw_variance: prediction.draw_variance * ratio * ratio,
        })
    };
    Profile::ALL
        .into_iter()
        .map(|profile| {
            let draw = learned(profile).or_else(|| scaled_from_active(profile));
            (profile, depletion(reading.energy_j, draw))
        })
        .collect()
}

fn observed_draw(draw_w: f64) -> Prediction {
    Prediction {
        draw_w,
        draw_variance: 0.0,
    }
}

fn depletion(energy_j: Option<f64>, draw: Option<Prediction>) -> Option<Estimate> {
    let energy = energy_j.filter(|e| *e > 0.0)?;
    let Prediction {
        draw_w,
        draw_variance,
    } = draw.filter(|prediction| prediction.draw_w > 0.0)?;
    Some(Estimate {
        time: whole_minutes(energy / draw_w),
        band: whole_minutes(energy / (draw_w * draw_w) * draw_variance.sqrt()),
    })
}

fn whole_minutes(seconds: f64) -> Duration {
    Duration::from_secs_f64((seconds / 60.0).round() * 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    fn hrs(duration: Duration) -> f64 {
        duration.as_secs_f64() / 3600.0
    }

    fn get(estimates: &HashMap<Profile, Option<Estimate>>, profile: Profile) -> Estimate {
        estimates
            .get(&profile)
            .copied()
            .flatten()
            .expect("estimate present")
    }

    fn balanced(model: RegressionState) -> HashMap<Profile, RegressionState> {
        HashMap::from([(Profile::Balanced, model)])
    }

    #[test]
    fn recovers_known_coefficients() {
        let true_draw = |c: Conditions| 5.0 + 8.0 * c.brightness_frac + 40.0 * c.cpu_busy_frac;
        let samples = [
            Conditions::new(0.1, 0.2),
            Conditions::new(0.9, 0.1),
            Conditions::new(0.3, 0.8),
            Conditions::new(0.6, 0.5),
            Conditions::new(0.2, 0.4),
            Conditions::new(0.8, 0.9),
        ];
        let mut model = RegressionState::seed(true_draw(samples[0]));
        for _ in 0..15 {
            for conditions in samples {
                model = model.observe(conditions, true_draw(conditions));
            }
        }
        assert!(
            (model.coefficients[0] - 5.0).abs() < 1.0,
            "base {}",
            model.coefficients[0]
        );
        assert!(
            (model.coefficients[1] - 8.0).abs() < 2.0,
            "k_bright {}",
            model.coefficients[1]
        );
        assert!(
            (model.coefficients[2] - 40.0).abs() < 2.0,
            "k_load {}",
            model.coefficients[2]
        );
    }

    #[test]
    fn responds_to_brightness_without_relearning() {
        let true_draw = |c: Conditions| 5.0 + 20.0 * c.brightness_frac;
        let dim = Conditions::new(0.1, 0.0);
        let bright = Conditions::new(0.9, 0.0);
        let mut model = RegressionState::seed(true_draw(dim));
        for _ in 0..80 {
            model = model.observe(dim, true_draw(dim));
            model = model.observe(bright, true_draw(bright));
        }
        let models = balanced(model);
        let life = |conditions| {
            let reading = PowerProfile::on_battery(Profile::Balanced, 50.0).at(conditions);
            hrs(get(&estimate(&reading, &models), Profile::Balanced).time)
        };
        assert!(
            life(bright) < life(dim),
            "a brighter screen should shorten predicted life: {} vs {}",
            life(bright),
            life(dim)
        );
    }

    #[test]
    fn band_shrinks_as_filter_learns() {
        let conditions = Conditions::new(0.5, 0.3);
        let after_one = RegressionState::seed(10.0).observe(conditions, 10.0);
        let mut after_many = RegressionState::seed(10.0);
        for _ in 0..50 {
            after_many = after_many.observe(conditions, 10.0);
        }
        let band = |model| {
            let reading = PowerProfile::on_battery(Profile::Balanced, 50.0).at(conditions);
            hrs(get(&estimate(&reading, &balanced(model)), Profile::Balanced).band)
        };
        assert!(
            band(after_many) < band(after_one),
            "band should tighten with more samples: {} vs {}",
            band(after_many),
            band(after_one)
        );
    }

    #[test]
    fn cold_start_matches_observed_draw() {
        let reading =
            PowerProfile::on_battery(Profile::Balanced, 60.0).at(Conditions::new(0.7, 0.4));
        let estimates = estimate(&reading, &balanced(RegressionState::seed(12.0)));
        assert!((hrs(get(&estimates, Profile::Balanced).time) - 5.0).abs() < 0.01);
    }

    #[test]
    fn scales_unobserved_profiles() {
        let reading =
            PowerProfile::on_battery(Profile::Balanced, 50.0).at(Conditions::new(0.0, 0.0));
        let estimates = estimate(&reading, &balanced(RegressionState::seed(10.0)));
        let saver = hrs(get(&estimates, Profile::PowerSaver).time);
        let balanced_life = hrs(get(&estimates, Profile::Balanced).time);
        let performance = hrs(get(&estimates, Profile::Performance).time);
        assert!(
            (performance - 50.0 / 15.0).abs() < 0.05,
            "performance {performance}"
        );
        assert!((saver - 50.0 / 6.0).abs() < 0.05, "saver {saver}");
        assert!(
            performance < balanced_life && balanced_life < saver,
            "life should order performance < balanced < power-saver"
        );
    }

    #[test]
    fn without_history_is_unknown() {
        let reading =
            PowerProfile::on_battery(Profile::Balanced, 50.0).at(Conditions::new(0.5, 0.5));
        let estimates = estimate(&reading, &HashMap::new());
        assert!(estimates[&Profile::Balanced].is_none());
    }

    #[test]
    fn missing_backlight_leaves_brightness_coefficient_inert() {
        let mut model = RegressionState::seed(10.0);
        for step in 0..60 {
            let cpu_busy_frac = if step % 2 == 0 { 0.2 } else { 0.8 };
            let draw = 10.0 + 30.0 * cpu_busy_frac;
            model = model.observe(Conditions::new(0.0, cpu_busy_frac), draw);
        }
        assert!(
            model.coefficients[1].abs() < 0.5,
            "k_bright {} should stay near its seed of 0",
            model.coefficients[1]
        );
        assert!(
            model.covariance[(1, 1)] > SEED_VARIANCE_COEFF,
            "k_bright variance {} should not shrink without brightness variation",
            model.covariance[(1, 1)]
        );
        assert!(
            (model.coefficients[2] - 30.0).abs() < 2.0,
            "k_load {} should still be learned",
            model.coefficients[2]
        );
    }
}
