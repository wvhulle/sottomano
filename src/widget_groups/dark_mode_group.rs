pub mod auto_switch_toggle;
pub mod umbra_dark_mode;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{TimeDelta, Utc};
use iced::{Alignment, Element, Subscription, Task, time::every, widget::Row};

use crate::{
    config::Config,
    data::{
        source::{
            umbra::{self, Umbra},
            weather::{self, Location},
        },
        sun_schedule::ColorMode,
    },
    message_subscriptions::blocking,
    state::{self, dark_mode::DarkModeSchedule},
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widget_groups::dark_mode_group::auto_switch_toggle::{AutoSwitchToggle, format_offset_minutes},
    widgets::{
        bar_widget::BarWidget,
        font_awesome::Solid,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{ToggleControl, ToggleMessage, toggle_button},
        tooltip_body::{Tooltip, row},
    },
};

const OFFSET_MINUTES: RangeInclusive<f32> = -120.0..=120.0;
const OFFSET_STEP_MINUTES: f32 = 5.0;
const SCHEDULE_CHECK: Duration = Duration::from_secs(30);

pub struct DarkModeGroup {
    mode_toggle: ToggleControl<Umbra>,
    auto_switch_toggle: AutoSwitchToggle,
    sunrise_offset_slider: SliderControl,
    sunset_offset_slider: SliderControl,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum DarkModeMessage {
    ScheduleLoaded(DarkModeSchedule),
    LocationFix(Result<Location, weather::Error>),
    ScheduleTick,
    ModeToggle(ToggleMessage<Umbra>),
    ToggleAutoSwitch,
    OffsetSlider(ColorMode, ControlMessage),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl Default for DarkModeGroup {
    fn default() -> Self {
        Self {
            mode_toggle: ToggleControl::default(),
            auto_switch_toggle: AutoSwitchToggle::default(),
            sunrise_offset_slider: new_offset_slider(HoverTarget::SunriseOffset, Solid::CloudSun),
            sunset_offset_slider: new_offset_slider(HoverTarget::SunsetOffset, Solid::CloudMoon),
            menu: MenuGroup::default(),
        }
    }
}

impl DarkModeGroup {
    pub fn probe() -> Task<DarkModeMessage> {
        Task::batch([
            ToggleControl::<Umbra>::probe(()).map(DarkModeMessage::ModeToggle),
            Task::perform(state::dark_mode::load(), DarkModeMessage::ScheduleLoaded),
        ])
    }

    pub fn subscription(&self) -> Subscription<DarkModeMessage> {
        let tick = if self.auto_switch_toggle.is_awaiting_switch() {
            every(SCHEDULE_CHECK).map(|_| DarkModeMessage::ScheduleTick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            self.mode_toggle
                .subscription(())
                .map(DarkModeMessage::ModeToggle),
            Subscription::run(Location::published).map(DarkModeMessage::LocationFix),
            tick,
        ])
    }

    pub fn is_animating(&self) -> bool {
        self.sunrise_offset_slider.is_animating() || self.sunset_offset_slider.is_animating()
    }

    fn close_controls(&mut self) {
        self.sunrise_offset_slider.close();
        self.sunset_offset_slider.close();
    }

    pub fn close_menu(&mut self) {
        if self.menu.close() {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: DarkModeMessage) -> Task<DarkModeMessage> {
        match msg {
            DarkModeMessage::ScheduleLoaded(saved) => {
                self.auto_switch_toggle
                    .restore_saved_schedule(saved, Utc::now());
                for edge in [ColorMode::Light, ColorMode::Dark] {
                    let minutes =
                        time_delta_to_minutes(saved.sun_event_offsets.offset_for_mode(edge));
                    self.offset_slider_for_mut(edge)
                        .update(ControlMessage::Reading(minutes));
                }
                Task::none()
            }
            DarkModeMessage::LocationFix(reading) => {
                if let Ok(location) = reading {
                    self.auto_switch_toggle
                        .update_location(location.position(), Utc::now());
                }
                Task::none()
            }
            DarkModeMessage::ScheduleTick => {
                let crossed = self.auto_switch_toggle.detect_mode_crossing(Utc::now());
                self.apply_color_mode(crossed)
            }
            DarkModeMessage::ModeToggle(m) => {
                self.mode_toggle.update(m).map(DarkModeMessage::ModeToggle)
            }
            DarkModeMessage::ToggleAutoSwitch => {
                let current = self.auto_switch_toggle.toggle_auto_switch(Utc::now());
                Task::batch([
                    self.persist_saved_schedule(),
                    self.apply_color_mode(current),
                ])
            }
            DarkModeMessage::OffsetSlider(edge, m) => self.update_offset_slider(edge, m),
            DarkModeMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.close_controls();
                }
                Task::none()
            }
            DarkModeMessage::Tooltip(_) => Task::none(),
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
                    .sun_event_offsets
                    .with_offset_for_mode(edge, minutes_to_time_delta(value));
                let passed = self
                    .auto_switch_toggle
                    .change_sun_event_offsets(offsets, Utc::now());
                let confirm = Task::future(state::dark_mode::save(
                    self.auto_switch_toggle.saved_schedule,
                ))
                .map(move |ok| {
                    DarkModeMessage::OffsetSlider(edge, ControlMessage::Confirmed { value, ok })
                });
                Task::batch([confirm, self.apply_color_mode(passed)])
            }
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn apply_color_mode(&mut self, mode: Option<ColorMode>) -> Task<DarkModeMessage> {
        let Some(mode) = mode else {
            return Task::none();
        };
        self.mode_toggle.assume(Umbra {
            dark: mode == ColorMode::Dark,
        });
        blocking("umbra set", move || umbra::set_mode(mode)).discard()
    }

    fn persist_saved_schedule(&self) -> Task<DarkModeMessage> {
        Task::future(state::dark_mode::save(
            self.auto_switch_toggle.saved_schedule,
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::DarkMode => self.mode_toggle.tooltip(),
            HoverTarget::DarkModeSchedule => Some(Tooltip::of(self.auto_switch_toggle.tooltip())),
            HoverTarget::SunriseOffset => Some(self.offset_slider_tooltip(ColorMode::Light)),
            HoverTarget::SunsetOffset => Some(self.offset_slider_tooltip(ColorMode::Dark)),
            _ => None,
        }
    }

    fn offset_slider_tooltip(&self, edge: ColorMode) -> Tooltip {
        Tooltip::table(vec![row(
            &format!("{} offset", edge.sun_event_label()),
            self.offset_slider_for(edge).readout_text(),
        )])
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, DarkModeMessage>> {
        let mode_reading = self.mode_toggle.reading()?;

        let children = move || {
            let mut children: Vec<Element<'a, DarkModeMessage>> = Vec::new();
            if let Some(mode) = self.mode_toggle.view(config, HoverTarget::DarkMode, 1.0) {
                children.push(mode.map(|m| match m {
                    ToggleMessage::Tooltip(t) => DarkModeMessage::Tooltip(t),
                    m => DarkModeMessage::ModeToggle(m),
                }));
            }
            children.extend(toggle_button(
                config,
                &self.auto_switch_toggle,
                HoverTarget::DarkModeSchedule,
                1.0,
                DarkModeMessage::ToggleAutoSwitch,
                DarkModeMessage::Tooltip,
            ));
            children.push(self.offset_slider_view(config, ColorMode::Light));
            children.push(self.offset_slider_view(config, ColorMode::Dark));
            children
        };

        let content = move |_reveal: f32| {
            Row::with_children(children())
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };
        Some(
            self.menu
                .slot(move || Self::face(config), content, DarkModeMessage::Menu)
                .previewed_value(mode_reading)
                .view(config, position),
        )
    }

    fn offset_slider_view<'a>(
        &self,
        config: &Config,
        edge: ColorMode,
    ) -> Element<'a, DarkModeMessage> {
        self.offset_slider_for(edge)
            .view(config, 1.0)
            .map(move |m| match m {
                ControlMessage::Tooltip(t) => DarkModeMessage::Tooltip(t),
                m => DarkModeMessage::OffsetSlider(edge, m),
            })
    }

    fn face<'a>(config: &Config) -> Element<'a, DarkModeMessage> {
        Solid::CircleHalfStroke
            .glyph()
            .size(config.font.icon_size_px)
            .color(config.palette.bar_foreground)
            .into()
    }
}

fn new_offset_slider(target: HoverTarget, icon: Solid) -> SliderControl {
    SliderControl::new(OFFSET_MINUTES, target, icon.into())
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
