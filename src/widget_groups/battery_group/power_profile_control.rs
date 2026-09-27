use std::{collections::HashMap, ops::RangeInclusive, time::Duration};

use iced::{Element, Subscription, Task};

pub mod profile_icon;

use crate::{
    config::Config,
    data::{
        battery_drain::{Estimate, RegressionState, estimate},
        source::{
            Source, bus,
            power_profile::{self, Conditions, PowerProfile, Profile},
        },
    },
    message_subscriptions::polling,
    state::power,
    wayland_layers::tooltip::{HoverTarget, TooltipMessage},
    widget_groups::battery_group::power_profile_control::profile_icon::profile_face,
    widgets::{
        last_poll::LastPoll,
        slider_control::{Action, ControlMessage, SliderControl},
        tooltip_body::{Cell, TableRow, Tooltip, TooltipBody},
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 50.0;
const MIN_SAMPLE_W: f64 = 0.5;

pub struct PowerProfileControl {
    cell: LastPoll<PowerProfile>,
    control: SliderControl,
    profile_models: HashMap<Profile, RegressionState>,
    estimates: HashMap<Profile, Option<Estimate>>,
    battery_id: String,
    load_started: bool,
    loaded: bool,
    daemon: bool,
}

impl Default for PowerProfileControl {
    fn default() -> Self {
        Self {
            cell: LastPoll::default(),
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
            profile_models: HashMap::new(),
            estimates: HashMap::new(),
            battery_id: String::new(),
            load_started: false,
            loaded: false,
            daemon: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PowerProfileMessage {
    Daemon(bool),
    Reading(Result<PowerProfile, bus::Error>),
    Loaded(HashMap<Profile, RegressionState>),
    Control(ControlMessage),
    Tooltip(TooltipMessage),
}

impl PowerProfileControl {
    pub fn is_animating(&self) -> bool {
        self.control.is_animating()
    }

    pub fn probe() -> Task<PowerProfileMessage> {
        Task::future(async { PowerProfileMessage::Daemon(PowerProfile::available(&()).await) })
    }

    pub fn subscription(&self) -> Subscription<PowerProfileMessage> {
        if !self.daemon {
            return Subscription::none();
        }
        polling::<PowerProfile>(()).map(PowerProfileMessage::Reading)
    }

    pub fn update(&mut self, msg: PowerProfileMessage) -> Task<PowerProfileMessage> {
        match msg {
            PowerProfileMessage::Daemon(daemon) => {
                self.daemon = daemon;
                Task::none()
            }
            PowerProfileMessage::Reading(reading) => {
                self.cell.receive(reading);
                let Some(PowerProfile {
                    active_profile,
                    draw_w,
                    discharging,
                    battery_id,
                    conditions,
                    ..
                }) = self.cell.get().cloned()
                else {
                    return Task::none();
                };
                self.control
                    .update(ControlMessage::Reading(active_profile.position()));

                let mut tasks = Vec::new();
                if !self.load_started && !battery_id.is_empty() {
                    self.load_started = true;
                    self.battery_id.clone_from(&battery_id);
                    tasks.push(
                        Task::future(power::load(battery_id)).map(PowerProfileMessage::Loaded),
                    );
                }
                if self.loaded {
                    let sample = draw_w.filter(|w| discharging && *w > MIN_SAMPLE_W);
                    tasks.extend(self.learn(active_profile, conditions, sample));
                }
                self.recompute();
                Task::batch(tasks)
            }
            PowerProfileMessage::Loaded(models) => {
                self.profile_models = models;
                self.loaded = true;
                self.recompute();
                Task::none()
            }
            PowerProfileMessage::Control(msg) => match self.control.update(msg) {
                Action::Commit(value) => {
                    let profile = Profile::from_position(value);
                    ControlMessage::commit("power profile set", value, move || {
                        power_profile::set(profile)
                    })
                    .map(PowerProfileMessage::Control)
                }
                Action::Opened | Action::None | Action::Right | Action::Adjust(_) => Task::none(),
            },
            PowerProfileMessage::Tooltip(_) => Task::none(),
        }
    }

    fn learn(
        &mut self,
        active_profile: Profile,
        conditions: Conditions,
        sample: Option<f64>,
    ) -> Option<Task<PowerProfileMessage>> {
        let continued = self.profile_models.remove(&active_profile);
        for model in self.profile_models.values_mut() {
            *model = model.interrupted();
        }
        let Some(draw) = sample else {
            if let Some(model) = continued {
                self.profile_models
                    .insert(active_profile, model.interrupted());
            }
            return None;
        };
        let next = continued
            .unwrap_or_else(|| RegressionState::seed(draw))
            .observe(conditions, draw);
        self.profile_models.insert(active_profile, next);
        Some(Task::future(power::save(self.battery_id.clone(), active_profile, next)).discard())
    }

    fn recompute(&mut self) {
        let Some(reading) = self.cell.get().cloned() else {
            return;
        };
        self.estimates = estimate(&reading, &self.profile_models);
        let caption = self
            .estimates
            .get(&reading.active_profile)
            .copied()
            .flatten()
            .map(|e| humantime::format_duration(e.time).to_string());
        self.control.update(ControlMessage::Caption(caption));
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        if !matches!(target, HoverTarget::PowerProfile) {
            return None;
        }
        let active_profile = Profile::from_position(self.control.current());
        self.cell
            .tooltip_with(TooltipBody::table(Profile::ALL.map(|profile| {
                ProfileLife {
                    profile,
                    active: profile == active_profile,
                    estimate: self.estimates.get(&profile).copied().flatten(),
                }
            })))
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        opacity: f32,
    ) -> Option<Element<'a, PowerProfileMessage>> {
        self.cell.get()?;
        Some(self.control.view(config, opacity).map(|m| match m {
            ControlMessage::Tooltip(t) => PowerProfileMessage::Tooltip(t),
            m => PowerProfileMessage::Control(m),
        }))
    }
}

fn duration_cell(duration: Duration) -> Cell {
    Cell::Text(humantime::format_duration(duration).to_string())
}

struct ProfileLife {
    profile: Profile,
    active: bool,
    estimate: Option<Estimate>,
}

impl TableRow for ProfileLife {
    const HEADINGS: &'static [&'static str] = &["Median", "Earliest", "Latest"];

    fn key(&self) -> Cell {
        if self.active {
            format!("{} (active)", self.profile.title()).into()
        } else {
            self.profile.title().into()
        }
    }

    fn cells(&self) -> Vec<Cell> {
        let Some(estimate) = self.estimate else {
            return vec!["—".into()];
        };
        vec![
            duration_cell(estimate.time),
            duration_cell(estimate.earliest),
            estimate.latest.map_or_else(|| "∞".into(), duration_cell),
        ]
    }
}
