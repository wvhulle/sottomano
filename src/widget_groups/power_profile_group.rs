//! odin's power-profile widget: a writable [`SliderControl`] selecting the
//! power-profiles-daemon profile (power-saver / balanced / performance). The
//! collapsed pill shows the profile glyph plus the estimated time to depletion;
//! committing a stop switches the profile via `powerprofilesctl`. Per-profile
//! discharge rates are learned by a Kalman filter and persisted to turso.

use std::{collections::HashMap, ops::RangeInclusive};

use iced::{Element, Subscription, Task};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{
        Tooltip, TooltipBody,
        power_profile::{
            self, Estimate, PowerProfile, Profile, SEED_VARIANCE, estimate, kalman_step,
        },
        row,
    },
    message_subscriptions::blocking,
    state::power::{self, RateState},
    wayland_layers::tooltip::{HoverTarget, TooltipMessage},
    widget_groups::source_cell::SourceCell,
    widgets::slider_control::{Action, ControlMessage, SliderControl},
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
/// Slider stop spacing: three profiles at 0 / 50 / 100.
const STEP: f32 = 50.0;
/// Below this draw (W) a sample is treated as noise/idle and not learned.
const MIN_SAMPLE_W: f64 = 0.5;

pub struct PowerProfileGroup {
    cell: SourceCell<PowerProfile>,
    control: SliderControl,
    states: HashMap<Profile, RateState>,
    estimates: HashMap<Profile, Option<Estimate>>,
    battery_id: String,
    load_started: bool,
    loaded: bool,
}

impl Default for PowerProfileGroup {
    fn default() -> Self {
        Self {
            cell: SourceCell::default(),
            control: SliderControl::new(RANGE, HoverTarget::PowerProfile, Profile::Balanced.icon())
                .slider_step(STEP)
                .scroll_step(STEP)
                .readout_width(70.0)
                .icon_only()
                .readout(|value| Profile::from_position(value).title().to_string())
                .glyph(|value, _| Profile::from_position(value).icon()),
            states: HashMap::new(),
            estimates: HashMap::new(),
            battery_id: String::new(),
            load_started: false,
            loaded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PowerProfileMessage {
    Reading(Option<PowerProfile>),
    Loaded(HashMap<Profile, RateState>),
    Control(ControlMessage),
    Tooltip(TooltipMessage),
}

impl PowerProfileGroup {
    pub fn subscription() -> Subscription<PowerProfileMessage> {
        SourceCell::subscription().map(PowerProfileMessage::Reading)
    }

    pub fn update(&mut self, msg: PowerProfileMessage) -> Task<PowerProfileMessage> {
        match msg {
            PowerProfileMessage::Reading(reading) => {
                self.cell.load(reading);
                let Some((active, draw_w, discharging, battery_id)) = self
                    .cell
                    .get()
                    .map(|p| (p.active, p.draw_w, p.discharging, p.battery_id.clone()))
                else {
                    return Task::none();
                };
                self.control
                    .update(ControlMessage::Reading(active.position()));

                let mut tasks = Vec::new();
                if !self.load_started && !battery_id.is_empty() {
                    self.load_started = true;
                    self.battery_id.clone_from(&battery_id);
                    tasks.push(
                        Task::future(power::load(battery_id)).map(PowerProfileMessage::Loaded),
                    );
                }
                if self.loaded
                    && discharging
                    && let Some(draw) = draw_w.filter(|w| *w > MIN_SAMPLE_W)
                {
                    let prior = self.states.get(&active).copied().unwrap_or(RateState {
                        rate_w: draw,
                        variance: SEED_VARIANCE,
                    });
                    let next = kalman_step(prior, draw);
                    self.states.insert(active, next);
                    tasks.push(
                        Task::future(power::save(self.battery_id.clone(), active, next)).discard(),
                    );
                }
                self.recompute();
                Task::batch(tasks)
            }
            PowerProfileMessage::Loaded(states) => {
                self.states = states;
                self.loaded = true;
                self.recompute();
                Task::none()
            }
            PowerProfileMessage::Control(msg) => match self.control.update(msg) {
                Action::Commit(value) => {
                    let profile = Profile::from_position(value);
                    blocking("power profile set", move || power_profile::set(profile)).map(
                        move |ok| {
                            PowerProfileMessage::Control(ControlMessage::Confirmed { value, ok })
                        },
                    )
                }
                Action::Opened | Action::None | Action::Right => Task::none(),
            },
            PowerProfileMessage::Tooltip(_) => Task::none(),
        }
    }

    fn recompute(&mut self) {
        let Some((active, energy_j, live_w)) = self
            .cell
            .get()
            .map(|p| (p.active, p.energy_j, p.draw_w.filter(|_| p.discharging)))
        else {
            return;
        };
        self.estimates = estimate(energy_j, active, &self.states, live_w);
        let caption = self
            .estimates
            .get(&active)
            .copied()
            .flatten()
            .map(|e| humantime::format_duration(e.time).to_string());
        self.control.update(ControlMessage::Caption(caption));
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        if !matches!(target, HoverTarget::PowerProfile) {
            return None;
        }
        let active = self.cell.get()?.active;
        let rows = Profile::ALL
            .into_iter()
            .map(|profile| {
                let key = if profile == active {
                    format!("{} (active)", profile.title())
                } else {
                    profile.title().to_string()
                };
                row(
                    &key,
                    estimate_text(self.estimates.get(&profile).copied().flatten()),
                )
            })
            .collect();
        self.cell.tooltip_with(TooltipBody::Table(rows))
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, PowerProfileMessage>> {
        self.cell.get()?;
        Some(self.control.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => PowerProfileMessage::Tooltip(t),
            m => PowerProfileMessage::Control(m),
        }))
    }
}

fn estimate_text(estimate: Option<Estimate>) -> String {
    let Some(estimate) = estimate else {
        return "—".to_string();
    };
    let time = humantime::format_duration(estimate.time);
    if estimate.band.as_secs() < 30 {
        time.to_string()
    } else {
        format!("{time} ±{}", humantime::format_duration(estimate.band))
    }
}
