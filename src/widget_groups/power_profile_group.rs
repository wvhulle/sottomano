use std::{collections::HashMap, ops::RangeInclusive};

use iced::{Element, Subscription, Task};

use crate::{
    config::Config,
    data::{
        last_poll::LastPoll,
        source::{
            Source, Tooltip, TooltipBody, bus,
            power_profile::{self, Estimate, PowerProfile, Profile, RegressionState, estimate},
            row,
        },
    },
    message_subscriptions::polling,
    state::power,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        pager_slot::permanently_inactive_slot,
        slider_control::{Action, ControlMessage, SliderControl},
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 50.0;
const MIN_SAMPLE_W: f64 = 0.5;

pub struct PowerProfileGroup {
    cell: LastPoll<PowerProfile>,
    control: SliderControl,
    profile_models: HashMap<Profile, RegressionState>,
    estimates: HashMap<Profile, Option<Estimate>>,
    battery_id: String,
    load_started: bool,
    loaded: bool,
    daemon: bool,
}

impl Default for PowerProfileGroup {
    fn default() -> Self {
        Self {
            cell: LastPoll::default(),
            control: SliderControl::new(RANGE, HoverTarget::PowerProfile, Profile::Balanced.icon())
                .slider_step(STEP)
                .scroll_step(STEP)
                .readout_width(70.0)
                .icon_only()
                .readout(|value| Profile::from_position(value).title().to_string())
                .glyph(|value, _| Profile::from_position(value).icon()),
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

impl PowerProfileGroup {
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
                if self.loaded
                    && discharging
                    && let Some(draw) = draw_w.filter(|w| *w > MIN_SAMPLE_W)
                {
                    let next = self
                        .profile_models
                        .get(&active_profile)
                        .copied()
                        .unwrap_or_else(|| RegressionState::seed(draw))
                        .observe(conditions, draw);
                    self.profile_models.insert(active_profile, next);
                    tasks.push(
                        Task::future(power::save(self.battery_id.clone(), active_profile, next))
                            .discard(),
                    );
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
        let rows = Profile::ALL
            .into_iter()
            .map(|profile| {
                let key = if profile == active_profile {
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

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, PowerProfileMessage>> {
        self.cell.get()?;
        let control = self.control.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => PowerProfileMessage::Tooltip(t),
            m => PowerProfileMessage::Control(m),
        });
        Some(permanently_inactive_slot(control, config, position))
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
