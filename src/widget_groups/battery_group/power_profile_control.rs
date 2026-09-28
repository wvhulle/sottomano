use std::{collections::HashMap, ops::RangeInclusive};

use iced::{Element, Subscription, Task};

pub mod profile_icon;

use crate::{
    config::Config,
    data::{
        battery_forecast::{Estimate, RegressionState, time_until_empty},
        source::{
            power_profile::{self, Conditions, Profile},
            sysfs_sensors::{self, battery::Battery},
        },
    },
    message_subscriptions::polling,
    state::power,
    wayland_layers::tooltip::{HoverTarget, TooltipMessage},
    widget_groups::battery_group::{
        estimate_columns::{ESTIMATE_HEADINGS, estimate_cells},
        power_profile_control::profile_icon::profile_face,
    },
    widgets::{
        slider_control::{Action, ControlMessage, SliderControl},
        tooltip_body::{Cell, TableRow, Tooltip, TooltipBody, row},
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 50.0;
const MIN_SAMPLE_W: f64 = 0.5;

pub struct PowerProfileControl {
    control: SliderControl,
    active_profile: Option<Profile>,
    conditions: Option<Conditions>,
    battery: Option<Battery>,
    profile_models: Option<HashMap<Profile, RegressionState>>,
    estimates: HashMap<Profile, Option<Estimate>>,
    load_started: bool,
    daemon: bool,
}

impl Default for PowerProfileControl {
    fn default() -> Self {
        Self {
            control: SliderControl::new(
                RANGE,
                HoverTarget::PowerProfile,
                profile_face(Profile::Balanced),
            )
            .slider_step(STEP)
            .scroll_step(STEP)
            .readout_width(70.0)
            .icon_only()
            .readout(|value| Profile::from_position(value).title().to_string())
            .glyph(|value, _| profile_face(Profile::from_position(value))),
            active_profile: None,
            conditions: None,
            battery: None,
            profile_models: None,
            estimates: HashMap::new(),
            load_started: false,
            daemon: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PowerProfileMessage {
    Daemon(bool),
    ActiveProfile(Profile),
    Conditions(Result<Conditions, sysfs_sensors::Error>),
    Loaded(HashMap<Profile, RegressionState>),
    Control(ControlMessage),
    Tooltip(TooltipMessage),
}

struct DrainSample {
    profile: Profile,
    conditions: Conditions,
    draw_w: f64,
}

impl PowerProfileControl {
    pub fn is_animating(&self) -> bool {
        self.control.is_animating()
    }

    pub fn probe() -> Task<PowerProfileMessage> {
        Task::future(power_profile::available()).map(PowerProfileMessage::Daemon)
    }

    pub fn subscription(&self) -> Subscription<PowerProfileMessage> {
        if !self.daemon {
            return Subscription::none();
        }
        Subscription::batch([
            Subscription::run(power_profile::active_profiles)
                .map(PowerProfileMessage::ActiveProfile),
            polling::<Conditions>(()).map(PowerProfileMessage::Conditions),
        ])
    }

    pub fn update(&mut self, msg: PowerProfileMessage) -> Task<PowerProfileMessage> {
        match msg {
            PowerProfileMessage::Daemon(daemon) => self.daemon = daemon,
            PowerProfileMessage::ActiveProfile(profile) => {
                self.active_profile = Some(profile);
                self.control
                    .update(ControlMessage::Reading(profile.position()));
                self.recompute();
            }
            PowerProfileMessage::Conditions(conditions) => {
                self.conditions = conditions
                    .inspect_err(|error| tracing::debug!("power conditions: {error}"))
                    .ok();
            }
            PowerProfileMessage::Loaded(models) => {
                self.profile_models = Some(models);
                self.recompute();
            }
            PowerProfileMessage::Control(msg) => {
                return match self.control.update(msg) {
                    Action::Commit(value) => {
                        let profile = Profile::from_position(value);
                        ControlMessage::commit("power profile set", value, move || {
                            power_profile::set(profile)
                        })
                        .map(PowerProfileMessage::Control)
                    }
                    Action::Opened | Action::None | Action::Right | Action::Adjust(_) => {
                        Task::none()
                    }
                };
            }
            PowerProfileMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    pub fn observe_battery(&mut self, battery: &Battery) -> Task<PowerProfileMessage> {
        let task = self.learn(battery);
        self.battery = Some(battery.clone());
        self.recompute();
        task
    }

    fn drain_sample(&self, battery: &Battery) -> Option<DrainSample> {
        Some(DrainSample {
            profile: self.active_profile?,
            conditions: self.conditions?,
            draw_w: battery
                .power_w
                .filter(|w| battery.is_discharging() && *w > MIN_SAMPLE_W)?,
        })
    }

    fn learn(&mut self, battery: &Battery) -> Task<PowerProfileMessage> {
        let sample = self.drain_sample(battery);
        let Some(models) = self.profile_models.as_mut() else {
            return self.start_loading(&battery.serial);
        };
        let continued = sample
            .as_ref()
            .and_then(|sample| models.remove(&sample.profile));
        for model in models.values_mut() {
            *model = model.interrupted();
        }
        let Some(sample) = sample else {
            return Task::none();
        };
        let next = continued
            .unwrap_or_else(|| RegressionState::seed(sample.draw_w))
            .observe(sample.conditions, sample.draw_w);
        models.insert(sample.profile, next);
        Task::future(power::save(battery.serial.clone(), sample.profile, next)).discard()
    }

    fn start_loading(&mut self, serial: &str) -> Task<PowerProfileMessage> {
        if self.load_started || serial.is_empty() {
            return Task::none();
        }
        self.load_started = true;
        Task::future(power::load(serial.to_string())).map(PowerProfileMessage::Loaded)
    }

    fn drain_estimates(&self) -> Option<HashMap<Profile, Option<Estimate>>> {
        let battery = self
            .battery
            .as_ref()
            .filter(|battery| battery.is_discharging())?;
        Some(time_until_empty(
            battery,
            self.active_profile?,
            self.conditions?,
            self.profile_models.as_ref()?,
        ))
    }

    fn recompute(&mut self) {
        self.estimates = self.drain_estimates().unwrap_or_default();
    }

    pub fn active_estimate(&self) -> Option<Estimate> {
        self.estimates.get(&self.active_profile?).copied().flatten()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        if !matches!(target, HoverTarget::PowerProfile) {
            return None;
        }
        self.active_profile?;
        let active_profile = Profile::from_position(self.control.current());
        let lives = Profile::ALL.map(|profile| ProfileLife {
            profile,
            active: profile == active_profile,
            estimate: self.estimates.get(&profile).copied().flatten(),
        });
        Some(Tooltip::of(if self.estimates.is_empty() {
            TooltipBody::rows(lives.map(|life| row(&life.title(), "")).into())
        } else {
            TooltipBody::table(lives)
        }))
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        opacity: f32,
    ) -> Option<Element<'a, PowerProfileMessage>> {
        self.active_profile?;
        Some(self.control.view(config, opacity).map(|m| match m {
            ControlMessage::Tooltip(t) => PowerProfileMessage::Tooltip(t),
            m => PowerProfileMessage::Control(m),
        }))
    }
}

struct ProfileLife {
    profile: Profile,
    active: bool,
    estimate: Option<Estimate>,
}

impl ProfileLife {
    fn title(&self) -> String {
        if self.active {
            format!("{} (active)", self.profile.title())
        } else {
            self.profile.title().to_string()
        }
    }
}

impl TableRow for ProfileLife {
    const HEADINGS: &'static [&'static str] = ESTIMATE_HEADINGS;

    fn key(&self) -> Cell {
        self.title().into()
    }

    fn cells(&self) -> Vec<Cell> {
        estimate_cells(self.estimate)
    }
}
