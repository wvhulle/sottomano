use crate::{
    group_items::reading_display_state::ReadingDisplayState, widgets::icon_glyph::IconGlyph,
};
mod auto_switch_toggle;
mod offset_slider;
mod sqlite_dark_mode_schedule;
mod umbra_dark_mode;
use std::{ops::RangeInclusive, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use iced::{Function, Subscription, Task, task};
use sottomano_desktop_sinks::screen_brightness_color::umbra_mode_writes::set_mode;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    blocking_background_work::blocking,
    login_session_presence::logind_manager,
    screen_brightness_color::{solar_switch_schedule::ColorMode, umbra_dark_mode::DarkMode},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    weather::situo_location_fix::Location,
};
use sottomano_toml_config::Config;
use tokio::time::sleep;
use umbra_dark_mode::dark_mode_reading_row;

use self::{
    offset_slider::{minutes_to_time_delta, new_offset_slider, time_delta_to_minutes},
    sqlite_dark_mode_schedule::DarkModeSchedule,
};
use crate::{
    background_work::{first_delivery, logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{ToggleControl, ToggleMessage},
    },
    widget_groups::{
        dark_mode_group::auto_switch_toggle::{AutoSwitchToggle, ScheduleStatus, SwitchDecision},
        group_summary::GroupSummary,
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::CommandOutcome,
        weather_icons::WeatherIcon,
    },
};

const OFFSET_MINUTES: RangeInclusive<f32> = -120.0..=120.0;
const OFFSET_STEP_MINUTES: f32 = 5.0;
const SWITCH_TIMER_MARGIN: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DarkModeSource {
    Mode,
    Location,
}

pub struct DarkModeGroup {
    mode_toggle: ToggleControl<DarkMode>,
    auto_switch_toggle: AutoSwitchToggle,
    sunrise_offset_slider: SliderControl,
    sunset_offset_slider: SliderControl,
    armed_switch: Option<ArmedSwitch>,
    expansion: WidgetGroupExpansion,
}

struct ArmedSwitch {
    at: DateTime<Utc>,
    _timer: task::Handle,
}

#[derive(Debug)]
pub enum DarkModeMessage {
    ScheduleLoaded(DarkModeSchedule),
    LocationFix(SnapshotDelivery<Location>),
    SwitchDue,
    ModeToggle(ToggleMessage<DarkMode>),
    OffsetPersisted(ColorMode, ControlMessage),
}

#[derive(Debug, Clone)]
pub enum DarkModeInteraction {
    ModeTogglePressed,
    ToggleAutoSwitch,
    OffsetSlider(ColorMode, ControlMessage),
    Expansion(WidgetGroupExpansionMessage),
    SwitchScheme(ColorMode),
    Refresh(DarkModeSource),
}

impl DarkModeGroup {
    #[must_use]
    pub fn new(clock: Clock) -> Self {
        Self {
            mode_toggle: ToggleControl::default(),
            auto_switch_toggle: AutoSwitchToggle::new(clock),
            sunrise_offset_slider: new_offset_slider("Sunrise offset", WeatherIcon::Sunrise),
            sunset_offset_slider: new_offset_slider("Sunset offset", WeatherIcon::Sunset),
            armed_switch: None,
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

impl DarkModeGroup {
    pub fn load() -> Task<DarkModeMessage> {
        Task::perform(
            sqlite_dark_mode_schedule::load(),
            DarkModeMessage::ScheduleLoaded,
        )
    }

    pub fn subscription() -> Subscription<DarkModeMessage> {
        Subscription::batch([
            ToggleControl::<DarkMode>::subscription(()).map(DarkModeMessage::ModeToggle),
            snapshot_subscription::<Location>(()).map(DarkModeMessage::LocationFix),
        ])
    }

    pub fn resume_subscription(buses: &DesktopBuses) -> Subscription<DarkModeMessage> {
        Subscription::run_with(buses.clone(), logind_manager::resumes)
            .map(|()| DarkModeMessage::SwitchDue)
    }

    fn refresh(source: DarkModeSource) -> Task<DarkModeMessage> {
        match source {
            DarkModeSource::Mode => {
                ToggleControl::<DarkMode>::refresh(()).map(DarkModeMessage::ModeToggle)
            }
            DarkModeSource::Location => {
                first_delivery::<Location>(()).map(DarkModeMessage::LocationFix)
            }
        }
    }

    #[must_use]
    pub fn scheme(&self) -> ColorMode {
        self.mode_toggle
            .snapshot()
            .map_or(ColorMode::Dark, |dark_mode| dark_mode.color_mode)
    }

    fn upcoming_switch(&self, clock: Clock) -> Option<DateTime<Utc>> {
        match self.auto_switch_toggle.status_at(clock.utc()) {
            ScheduleStatus::Following { upcoming, .. } => Some(upcoming.switch_time),
            ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => None,
        }
    }

    fn rearm_switch(&mut self, clock: Clock) -> Task<DarkModeMessage> {
        let upcoming = self.upcoming_switch(clock);
        if self.armed_switch.as_ref().map(|armed| armed.at) == upcoming {
            return Task::none();
        }
        let Some(at) = upcoming else {
            self.armed_switch = None;
            return Task::none();
        };
        let remaining = (at - clock.utc())
            .max(TimeDelta::zero())
            .to_std()
            .expect("a non-negative time delta fits a std duration");
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
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: DarkModeMessage, clock: Clock) -> Task<DarkModeMessage> {
        let handled = self.handle(msg, clock);
        Task::batch([handled, self.rearm_switch(clock)])
    }

    pub fn interact(
        &mut self,
        interaction: DarkModeInteraction,
        clock: Clock,
    ) -> Task<DarkModeMessage> {
        let handled = self.handle_interaction(interaction);
        Task::batch([handled, self.rearm_switch(clock)])
    }

    fn handle_interaction(&mut self, interaction: DarkModeInteraction) -> Task<DarkModeMessage> {
        match interaction {
            DarkModeInteraction::ModeTogglePressed => {
                self.mode_toggle.press().map(DarkModeMessage::ModeToggle)
            }
            DarkModeInteraction::ToggleAutoSwitch => {
                self.auto_switch_toggle.toggle_auto_switch();
                self.persist_saved_schedule()
            }
            DarkModeInteraction::OffsetSlider(edge, control_message) => {
                self.update_offset_slider(edge, control_message)
            }
            DarkModeInteraction::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.close_controls();
                }
                Task::none()
            }
            DarkModeInteraction::SwitchScheme(mode) => self.apply_color_mode(mode),
            DarkModeInteraction::Refresh(source) => Self::refresh(source),
        }
    }

    fn handle(&mut self, msg: DarkModeMessage, clock: Clock) -> Task<DarkModeMessage> {
        match msg {
            DarkModeMessage::ScheduleLoaded(saved) => {
                self.auto_switch_toggle.saved_schedule = saved;
                for edge in [ColorMode::Light, ColorMode::Dark] {
                    let minutes =
                        time_delta_to_minutes(saved.sun_event_offsets.offset_for_mode(edge));
                    self.offset_slider_for_mut(edge)
                        .update(ControlMessage::Reading(minutes));
                }
                Task::none()
            }
            DarkModeMessage::LocationFix(delivery) => {
                if let SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(location),
                    ..
                } = delivery
                {
                    self.auto_switch_toggle.update_location(location.position());
                }
                Task::none()
            }
            DarkModeMessage::SwitchDue => self.switch_if_due(clock.utc()),
            DarkModeMessage::ModeToggle(m) => self
                .mode_toggle
                .update(m, clock)
                .map(DarkModeMessage::ModeToggle),
            DarkModeMessage::OffsetPersisted(edge, control_message) => {
                self.update_offset_slider(edge, control_message)
            }
        }
    }

    fn update_offset_slider(
        &mut self,
        edge: ColorMode,
        control_message: ControlMessage,
    ) -> Task<DarkModeMessage> {
        match self.offset_slider_for_mut(edge).update(control_message) {
            Action::Opened => {
                self.offset_slider_for_mut(edge.opposite_mode()).close();
                Task::none()
            }
            Action::Commit(value) => {
                let offsets = self
                    .auto_switch_toggle
                    .saved_schedule
                    .sun_event_offsets
                    .with_offset_for_mode(edge, minutes_to_time_delta(value));
                self.auto_switch_toggle.change_sun_event_offsets(offsets);
                Task::future(sqlite_dark_mode_schedule::save(
                    self.auto_switch_toggle.saved_schedule,
                ))
                .map(move |store_outcome| {
                    DarkModeMessage::OffsetPersisted(
                        edge,
                        ControlMessage::persisted(value, store_outcome),
                    )
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
        self.mode_toggle.apply_command(CommandOutcome::Applied {
            expected_snapshot: DarkMode { color_mode: mode },
        });
        Task::future(blocking("umbra set", move || {
            if let Err(umbra_failure) = set_mode(mode) {
                tracing::warn!("umbra set {}: {umbra_failure}", mode.umbra_name());
            }
        }))
        .discard()
    }

    fn persist_saved_schedule(&self) -> Task<DarkModeMessage> {
        logged_failure_task(
            "dark mode schedule: save".to_owned(),
            sqlite_dark_mode_schedule::save(self.auto_switch_toggle.saved_schedule),
        )
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

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, DarkModeInteraction> {
        let locale = config.calendar.locale;
        let mode_known = self.mode_toggle.snapshot().is_some();
        let offset_slider = |edge| {
            GroupItem::slider(
                self.offset_slider_for(edge),
                config,
                DarkModeInteraction::OffsetSlider.with(edge),
            )
            .refreshed_by(DarkModeInteraction::Refresh(DarkModeSource::Location))
        };
        WidgetGroupContent::expandable(
            WidgetGroupFamily::DarkMode,
            GroupSummary::glyph(
                Phosphor::MoonStars(Weight::Regular).into(),
                ReadingDisplayState::Idle,
            ),
            &self.expansion,
            DarkModeInteraction::Expansion,
            [
                self.mode_toggle
                    .group_item(
                        IconGlyph::default(),
                        dark_mode_reading_row,
                        DarkModeInteraction::ModeTogglePressed,
                        clock,
                    )
                    .map(|mode_item| {
                        mode_item
                            .with_hover_card(Box::new(move |_| self.mode_toggle.hover_card()))
                            .refreshed_by(DarkModeInteraction::Refresh(DarkModeSource::Mode))
                    }),
                mode_known.then(|| {
                    GroupItem::reading(self.auto_switch_toggle.reading_row(clock, locale))
                        .on_switch_press(
                            self.auto_switch_toggle.saved_schedule.auto_switch_arming,
                            self.auto_switch_toggle.press_does(),
                            DarkModeInteraction::ToggleAutoSwitch,
                        )
                        .refreshed_by(DarkModeInteraction::Refresh(DarkModeSource::Location))
                }),
                mode_known.then(|| offset_slider(ColorMode::Light)),
                mode_known.then(|| offset_slider(ColorMode::Dark)),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
