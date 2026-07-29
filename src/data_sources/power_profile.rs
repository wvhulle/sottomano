//! The active power profile (power-profiles-daemon), read and set via
//! `powerprofilesctl` exactly as odin's `power-profile.nu` does. Rendered as a
//! writable [`SliderControl`](crate::widgets::slider_control::SliderControl)
//! selector; the reading also carries the battery's live energy/draw for the
//! per-profile time-to-depletion estimate, learned by a Kalman filter.

use std::{collections::HashMap, time::Duration};

use adskalman::{
    KalmanFilterNoControl, ObservationModel, StateAndCovariance, TransitionModelLinearNoControl,
};
use nalgebra::{Matrix1, U1, Vector1};

use crate::{
    data_sources::{BarWidget, Icon, Source, TooltipBody, command, sysfs_sensors},
    state::power::RateState,
};

/// The three power-profiles-daemon profiles, least-to-most performant.
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

    pub fn icon(self) -> Icon {
        match self {
            Profile::PowerSaver => Icon::Leaf,
            Profile::Balanced => Icon::ScaleBalanced,
            Profile::Performance => Icon::Rocket,
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

pub fn active() -> Option<Profile> {
    Profile::from_name(&command::output("powerprofilesctl", &["get"])?)
}

pub fn set(profile: Profile) -> bool {
    command::run("powerprofilesctl", &["set", profile.name()])
}

#[derive(Debug, Clone)]
pub struct PowerProfile {
    pub active: Profile,
    pub energy_j: Option<f64>,
    pub draw_w: Option<f64>,
    pub discharging: bool,
    pub battery_id: String,
}

impl BarWidget for PowerProfile {
    fn icon(&self) -> Icon {
        self.active.icon()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

impl Source for PowerProfile {
    const NAME: &'static str = "power-profile";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn poll() -> Option<Self> {
        let active = active()?;
        let energy = sysfs_sensors::battery_energy();
        Some(PowerProfile {
            active,
            energy_j: energy.as_ref().and_then(|e| e.energy_j),
            draw_w: energy.as_ref().and_then(|e| e.draw_w),
            discharging: energy.as_ref().is_some_and(|e| e.discharging),
            battery_id: energy.map(|e| e.serial).unwrap_or_default(),
        })
    }
}

/// Process/measurement noise (W²) and the variance a fresh profile starts at —
/// large, so the first sample is trusted before the filter tightens.
const PROCESS_NOISE: f64 = 0.25;
const MEASUREMENT_NOISE: f64 = 4.0;
pub const SEED_VARIANCE: f64 = 100.0;

struct RandomWalk {
    f: Matrix1<f64>,
    q: Matrix1<f64>,
}

impl TransitionModelLinearNoControl<f64, U1> for RandomWalk {
    fn F(&self) -> &Matrix1<f64> {
        &self.f
    }
    fn FT(&self) -> &Matrix1<f64> {
        &self.f
    }
    fn Q(&self) -> &Matrix1<f64> {
        &self.q
    }
}

struct DirectObservation {
    h: Matrix1<f64>,
    r: Matrix1<f64>,
}

impl ObservationModel<f64, U1, U1> for DirectObservation {
    fn H(&self) -> &Matrix1<f64> {
        &self.h
    }
    fn HT(&self) -> &Matrix1<f64> {
        &self.h
    }
    fn R(&self) -> &Matrix1<f64> {
        &self.r
    }
}

pub fn kalman_step(prior: RateState, observed_w: f64) -> RateState {
    let transition = RandomWalk {
        f: Matrix1::identity(),
        q: Matrix1::new(PROCESS_NOISE),
    };
    let observation = DirectObservation {
        h: Matrix1::identity(),
        r: Matrix1::new(MEASUREMENT_NOISE),
    };
    let filter = KalmanFilterNoControl::new(&transition, &observation);
    let estimate =
        StateAndCovariance::new(Vector1::new(prior.rate_w), Matrix1::new(prior.variance));
    filter
        .step(&estimate, &Vector1::new(observed_w))
        .map_or(prior, |posterior| RateState {
            rate_w: posterior.state()[0],
            variance: posterior.covariance()[(0, 0)],
        })
}

/// Time to depletion for a profile, with a ± uncertainty band from the rate's
/// posterior variance propagated through `energy / rate`.
#[derive(Debug, Clone, Copy)]
pub struct Estimate {
    pub time: Duration,
    pub band: Duration,
}

pub fn estimate(
    energy_j: Option<f64>,
    active: Profile,
    states: &HashMap<Profile, RateState>,
    live_w: Option<f64>,
) -> HashMap<Profile, Option<Estimate>> {
    let active_rate = states
        .get(&active)
        .map(|s| s.rate_w)
        .filter(|r| *r > 0.0)
        .or_else(|| live_w.filter(|w| *w > 0.0));
    Profile::ALL
        .into_iter()
        .map(|profile| {
            (
                profile,
                estimate_one(energy_j, active, active_rate, states, profile),
            )
        })
        .collect()
}

fn estimate_one(
    energy_j: Option<f64>,
    active: Profile,
    active_rate: Option<f64>,
    states: &HashMap<Profile, RateState>,
    profile: Profile,
) -> Option<Estimate> {
    let energy = energy_j.filter(|e| *e > 0.0)?;
    let (rate, variance) = match states.get(&profile) {
        Some(state) if state.rate_w > 0.0 => (state.rate_w, state.variance),
        _ => {
            let scaled = active_rate? * profile.scale() / active.scale();
            (scaled, scaled * scaled)
        }
    };
    (rate > 0.0).then(|| Estimate {
        time: Duration::from_secs_f64(energy / rate),
        band: Duration::from_secs_f64(energy / (rate * rate) * variance.sqrt()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(rate_w: f64) -> RateState {
        RateState {
            rate_w,
            variance: SEED_VARIANCE,
        }
    }

    fn hrs(duration: Duration) -> f64 {
        duration.as_secs_f64() / 3600.0
    }

    fn joules(watt_hours: f64) -> f64 {
        watt_hours * 3600.0
    }

    fn get(estimates: &HashMap<Profile, Option<Estimate>>, profile: Profile) -> Estimate {
        estimates
            .get(&profile)
            .copied()
            .flatten()
            .expect("estimate present")
    }

    #[test]
    fn kalman_converges_to_steady_draw() {
        let mut state = seed(10.0);
        for _ in 0..50 {
            state = kalman_step(state, 12.0);
        }
        assert!(
            (state.rate_w - 12.0).abs() < 0.5,
            "rate {} should approach the observed 12 W",
            state.rate_w
        );
        assert!(
            state.variance < SEED_VARIANCE,
            "variance {} should shrink below the seed",
            state.variance
        );
    }

    #[test]
    fn kalman_smooths_alternating_noise() {
        let mut state = seed(10.0);
        for i in 0..40 {
            let watts = if i % 2 == 0 { 6.0 } else { 14.0 };
            state = kalman_step(state, watts);
        }
        assert!(
            (state.rate_w - 10.0).abs() < 1.5,
            "smoothed rate {} should stay near 10 W",
            state.rate_w
        );
    }

    #[test]
    fn estimate_active_profile_time_and_band() {
        let states = HashMap::from([(
            Profile::Balanced,
            RateState {
                rate_w: 10.0,
                variance: 0.25,
            },
        )]);
        let estimates = estimate(Some(joules(50.0)), Profile::Balanced, &states, Some(10.0));
        let balanced = get(&estimates, Profile::Balanced);
        assert!((hrs(balanced.time) - 5.0).abs() < 0.01);
        assert!((hrs(balanced.band) - 0.25).abs() < 0.01);
    }

    #[test]
    fn estimate_scales_unobserved_profiles() {
        let states = HashMap::from([(
            Profile::Balanced,
            RateState {
                rate_w: 10.0,
                variance: 1.0,
            },
        )]);
        let estimates = estimate(Some(joules(50.0)), Profile::Balanced, &states, Some(10.0));
        let saver = hrs(get(&estimates, Profile::PowerSaver).time);
        let balanced = hrs(get(&estimates, Profile::Balanced).time);
        let performance = hrs(get(&estimates, Profile::Performance).time);
        assert!(
            (performance - 50.0 / 15.0).abs() < 0.05,
            "performance {performance}"
        );
        assert!((saver - 50.0 / 6.0).abs() < 0.05, "saver {saver}");
        assert!(
            performance < balanced && balanced < saver,
            "life should order performance < balanced < power-saver"
        );
    }

    #[test]
    fn estimate_on_ac_without_history_is_unknown() {
        let estimates = estimate(Some(joules(50.0)), Profile::Balanced, &HashMap::new(), None);
        assert!(estimates[&Profile::Balanced].is_none());
    }

    #[test]
    fn uncertainty_band_shrinks_as_filter_learns() {
        let after_one = kalman_step(seed(10.0), 10.0);
        let mut after_many = seed(10.0);
        for _ in 0..50 {
            after_many = kalman_step(after_many, 10.0);
        }
        let band = |state| {
            let states = HashMap::from([(Profile::Balanced, state)]);
            hrs(get(
                &estimate(Some(joules(50.0)), Profile::Balanced, &states, Some(10.0)),
                Profile::Balanced,
            )
            .band)
        };
        assert!(
            band(after_many) < band(after_one),
            "band should tighten with more samples: {} vs {}",
            band(after_many),
            band(after_one)
        );
    }
}
