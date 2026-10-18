mod auto_switch_toggle;
mod umbra_dark_mode;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use iced::{Function, Subscription, Task, task};
use tokio::time::sleep;
use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    background_work::{blocking, first_delivery, source_subscription},
    bus::DesktopBuses,
    logind,
    sun_schedule::ColorMode,
    umbra::{self, Umbra},
    weather::Location,
};
use vetrata_state::dark_mode::{self, DarkModeSchedule};

use crate::{
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::dark_mode_group::auto_switch_toggle::{
        AutoSwitchToggle, ScheduleStatus, SwitchDecision, format_offset_minutes,
    },
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::Tooltip,
        weather_icons::WeatherIcon,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

const OFFSET_MINUTES: RangeInclusive<f32> = -120.0..=120.0;
const OFFSET_STEP_MINUTES: f32 = 5.0;
const SWITCH_TIMER_MARGIN: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DarkModeTarget {
    Mode,
    Schedule,
    SunriseOffset,
    SunsetOffset,
}

impl From<DarkModeTarget> for HoverTarget {
    fn from(target: DarkModeTarget) -> Self {
        HoverTarget::DarkMode(target)
    }
}

pub struct DarkModeGroup {
    mode_toggle: ToggleControl<Umbra>,
    auto_switch_toggle: AutoSwitchToggle,
    sunrise_offset_slider: SliderControl,
    sunset_offset_slider: SliderControl,
    armed_switch: Option<ArmedSwitch>,
    expansion: GroupExpansion,
}

struct ArmedSwitch {
    at: DateTime<Utc>,
    _timer: task::Handle,
}

#[derive(Debug, Clone)]
pub enum DarkModeMessage {
    ScheduleLoaded(SourceDelivery<DarkModeSchedule>),
    LocationFix(SourceDelivery<Location>),
    SwitchDue,
    ModeToggle(ToggleMessage<Umbra>),
    ToggleAutoSwitch,
    OffsetSlider(ColorMode, ControlMessage),
    Expansion(ExpansionMessage),
}

impl Default for DarkModeGroup {
    fn default() -> Self {
        Self {
            mode_toggle: ToggleControl::default(),
            auto_switch_toggle: AutoSwitchToggle::default(),
            sunrise_offset_slider: new_offset_slider(
                "Sunrise offset",
                DarkModeTarget::SunriseOffset,
                WeatherIcon::Sunrise,
            ),
            sunset_offset_slider: new_offset_slider(
                "Sunset offset",
                DarkModeTarget::SunsetOffset,
                WeatherIcon::Sunset,
            ),
            armed_switch: None,
            expansion: GroupExpansion::default(),
        }
    }
}

impl DarkModeGroup {
    pub fn subscription() -> Subscription<DarkModeMessage> {
        Subscription::batch([
            ToggleControl::<Umbra>::subscription(()).map(DarkModeMessage::ModeToggle),
            source_subscription::<DarkModeSchedule>(()).map(DarkModeMessage::ScheduleLoaded),
            source_subscription::<Location>(()).map(DarkModeMessage::LocationFix),
        ])
    }

    pub fn resume_subscription(buses: &DesktopBuses) -> Subscription<DarkModeMessage> {
        Subscription::run_with(buses.clone(), logind::resumes).map(|()| DarkModeMessage::SwitchDue)
    }

    pub fn refresh(target: DarkModeTarget) -> Task<DarkModeMessage> {
        match target {
            DarkModeTarget::Mode => {
                ToggleControl::<Umbra>::refresh(()).map(DarkModeMessage::ModeToggle)
            }
            DarkModeTarget::Schedule
            | DarkModeTarget::SunriseOffset
            | DarkModeTarget::SunsetOffset => {
                first_delivery::<Location>(()).map(DarkModeMessage::LocationFix)
            }
        }
    }

    fn upcoming_switch(&self) -> Option<DateTime<Utc>> {
        match self.auto_switch_toggle.status_at(Utc::now()) {
            ScheduleStatus::Following { upcoming, .. } => Some(upcoming.switch_time),
            ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => None,
        }
    }

    fn rearm_switch(&mut self) -> Task<DarkModeMessage> {
        let upcoming = self.upcoming_switch();
        if self.armed_switch.as_ref().map(|armed| armed.at) == upcoming {
            return Task::none();
        }
        let Some(at) = upcoming else {
            self.armed_switch = None;
            return Task::none();
        };
        let remaining = (at - Utc::now()).to_std().unwrap_or_default();
        let (timer, handle) = Task::future(sleep(remaining + SWITCH_TIMER_MARGIN))
            .map(|()| DarkModeMessage::SwitchDue)
            .abortable();
        self.armed_switch = Some(ArmedSwitch {
            at,
            _timer: handle.abort_on_drop(),
        });
        timer
    }

    fn close_controls(&mut self) {
        self.sunrise_offset_slider.close();
        self.sunset_offset_slider.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: DarkModeMessage) -> Task<DarkModeMessage> {
        let handled = self.handle(msg);
        Task::batch([handled, self.rearm_switch()])
    }

    fn handle(&mut self, msg: DarkModeMessage) -> Task<DarkModeMessage> {
        match msg {
            DarkModeMessage::ScheduleLoaded(delivery) => {
                self.auto_switch_toggle
                    .saved_schedule
                    .receive_keeping_last_reading(delivery);
                let saved = self
                    .auto_switch_toggle
                    .saved_schedule
                    .get()
                    .copied()
                    .unwrap_or_default();
                for edge in [ColorMode::Light, ColorMode::Dark] {
                    let minutes =
                        time_delta_to_minutes(saved.sun_event_offsets.offset_for_mode(edge));
                    self.offset_slider_for_mut(edge)
                        .update(ControlMessage::Reading(minutes));
                }
                Task::none()
            }
            DarkModeMessage::LocationFix(delivery) => {
                if let SourceDelivery::Reading(Ok(location)) = delivery {
                    self.auto_switch_toggle.update_location(location.position());
                }
                Task::none()
            }
            DarkModeMessage::SwitchDue => self.switch_if_due(Utc::now()),
            DarkModeMessage::ModeToggle(m) => {
                self.mode_toggle.update(m).map(DarkModeMessage::ModeToggle)
            }
            DarkModeMessage::ToggleAutoSwitch => {
                self.auto_switch_toggle.toggle_auto_switch();
                self.persist_saved_schedule()
            }
            DarkModeMessage::OffsetSlider(edge, m) => self.update_offset_slider(edge, m),
            DarkModeMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_controls();
                }
                Task::none()
            }
        }
    }

    fn update_offset_slider(
        &mut self,
        edge: ColorMode,
        m: ControlMessage,
    ) -> Task<DarkModeMessage> {
        match self.offset_slider_for_mut(edge).update(m) {
            Action::Opened => {
                self.offset_slider_for_mut(edge.opposite_mode()).close();
                Task::none()
            }
            Action::Commit(value) => {
                let offsets = self
                    .auto_switch_toggle
                    .saved_schedule
                    .get()
                    .copied()
                    .unwrap_or_default()
                    .sun_event_offsets
                    .with_offset_for_mode(edge, minutes_to_time_delta(value));
                self.auto_switch_toggle.change_sun_event_offsets(offsets);
                Task::future(dark_mode::save(
                    self.auto_switch_toggle
                        .saved_schedule
                        .get()
                        .copied()
                        .unwrap_or_default(),
                ))
                .map(move |ok| {
                    DarkModeMessage::OffsetSlider(edge, ControlMessage::Persisted { value, ok })
                })
            }
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn switch_if_due(&mut self, now: DateTime<Utc>) -> Task<DarkModeMessage> {
        let decision = self.auto_switch_toggle.switch_decision_at(now);
        self.auto_switch_toggle.mark_checked(now);
        match decision {
            SwitchDecision::Apply(mode) => self.apply_color_mode(mode),
            SwitchDecision::Hold => Task::none(),
        }
    }

    fn apply_color_mode(&mut self, mode: ColorMode) -> Task<DarkModeMessage> {
        self.mode_toggle.assume(Umbra {
            dark: mode == ColorMode::Dark,
        });
        blocking("umbra set", move || umbra::set_mode(mode)).discard()
    }

    fn persist_saved_schedule(&self) -> Task<DarkModeMessage> {
        Task::future(dark_mode::save(
            self.auto_switch_toggle
                .saved_schedule
                .get()
                .copied()
                .unwrap_or_default(),
        ))
        .discard()
    }

    fn offset_slider_for(&self, edge: ColorMode) -> &SliderControl {
        match edge {
            ColorMode::Light => &self.sunrise_offset_slider,
            ColorMode::Dark => &self.sunset_offset_slider,
        }
    }

    fn offset_slider_for_mut(&mut self, edge: ColorMode) -> &mut SliderControl {
        match edge {
            ColorMode::Light => &mut self.sunrise_offset_slider,
            ColorMode::Dark => &mut self.sunset_offset_slider,
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: DarkModeTarget) -> Option<Tooltip> {
        match target {
            DarkModeTarget::Mode => self.mode_toggle.tooltip(),
            DarkModeTarget::Schedule => self.auto_switch_toggle.schedule_tooltip(),
            DarkModeTarget::SunriseOffset => Some(self.offset_slider_tooltip(ColorMode::Light)),
            DarkModeTarget::SunsetOffset => Some(self.offset_slider_tooltip(ColorMode::Dark)),
        }
    }

    fn offset_slider_tooltip(&self, edge: ColorMode) -> Tooltip {
        self.offset_slider_for(edge)
            .readout_tooltip(format!("{} offset", edge.sun_event_label()))
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, DarkModeMessage> {
        let mode_known = self.mode_toggle.reading().is_some();
        let offset_slider = |edge| {
            GroupItem::slider(
                self.offset_slider_for(edge),
                config,
                DarkModeMessage::OffsetSlider.with(edge),
            )
        };
        GroupContent::new(
            &self.expansion,
            DarkModeMessage::Expansion,
            [
                self.mode_toggle
                    .group_item(DarkModeTarget::Mode, DarkModeMessage::ModeToggle),
                mode_known.then(|| {
                    GroupItem::reading(&self.auto_switch_toggle, DarkModeTarget::Schedule).on_press(
                        self.auto_switch_toggle.press_does(),
                        DarkModeMessage::ToggleAutoSwitch,
                    )
                }),
                mode_known.then(|| offset_slider(ColorMode::Light)),
                mode_known.then(|| offset_slider(ColorMode::Dark)),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::MoonStars(Weight::Regular).into(),
            handle: GroupHandle::DarkMode,
        })
        .preview(self.mode_toggle.reading().map(PreviewedValue::Plain))
    }
}

fn new_offset_slider(
    name: &'static str,
    target: DarkModeTarget,
    icon: WeatherIcon,
) -> SliderControl {
    SliderControl::new(name, OFFSET_MINUTES, target, icon)
        .scroll_step(OFFSET_STEP_MINUTES)
        .slider_step(OFFSET_STEP_MINUTES)
        .readout(|minutes| format_offset_minutes(minutes_to_time_delta(minutes)))
}

fn minutes_to_time_delta(minutes: f32) -> TimeDelta {
    TimeDelta::minutes(minutes.round() as i64)
}

fn time_delta_to_minutes(offset: TimeDelta) -> f32 {
    offset.num_minutes() as f32
}
