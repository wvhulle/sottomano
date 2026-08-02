use std::{collections::BTreeMap, ops::RangeInclusive};

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        BarWidget, Tooltip, TooltipBody,
        backlight::{self, Backlight},
        clight::Clight,
        font_awesome::Solid,
        gamma::{self, Gamma},
        row,
        umbra::Umbra,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widget_groups::source_cell::SourceCell,
    widgets::{
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{ToggleControl, ToggleMessage},
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 5.0;
const WARMTH_RANGE: RangeInclusive<f32> = 3000.0..=6500.0;
const WARMTH_STEP: f32 = 100.0;

pub struct MonitorGroup {
    backlights: Vec<Backlight>,
    sliders: BTreeMap<String, SliderControl>,
    gamma: SourceCell<Gamma>,
    dim: SliderControl,
    warmth: SliderControl,
    clight: ToggleControl<Clight>,
    umbra: ToggleControl<Umbra>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Menu(MenuMessage),
    Backlights(Vec<Backlight>),
    Backlight(String, ControlMessage),
    Gamma(Option<Gamma>),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    Clight(ToggleMessage<Clight>),
    Umbra(ToggleMessage<Umbra>),
    Tooltip(TooltipMessage),
}

impl MonitorGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            backlights: Vec::new(),
            sliders: BTreeMap::new(),
            gamma: SourceCell::default(),
            dim: SliderControl::new(
                PERCENT,
                HoverTarget::GammaDim,
                Solid::CircleHalfStroke.into(),
            )
            .scroll_step(STEP)
            .readout(|value| format!("{}%", as_percent(value))),
            warmth: SliderControl::new(
                WARMTH_RANGE,
                HoverTarget::ColorTemperature,
                Solid::TemperatureHalf.into(),
            )
            .scroll_step(WARMTH_STEP)
            .slider_step(WARMTH_STEP)
            .readout_width(52.0)
            .readout(|value| format!("{}K", as_kelvin(value)))
            .label(|value| format!("{}K", as_kelvin(value))),
            clight: ToggleControl::default(),
            umbra: ToggleControl::default(),
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<MonitorMessage> {
        Subscription::batch([
            backlight::subscription().map(MonitorMessage::Backlights),
            SourceCell::<Gamma>::subscription().map(MonitorMessage::Gamma),
            ToggleControl::<Clight>::subscription().map(MonitorMessage::Clight),
            ToggleControl::<Umbra>::subscription().map(MonitorMessage::Umbra),
        ])
    }

    pub fn update(&mut self, msg: MonitorMessage) -> Task<MonitorMessage> {
        match msg {
            MonitorMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.close_backlight_sliders(None);
                    self.dim.close();
                    self.warmth.close();
                }
                Task::none()
            }
            MonitorMessage::Backlights(list) => {
                self.sliders
                    .retain(|serial, _| list.iter().any(|b| &b.serial == serial));
                for backlight in &list {
                    self.sliders
                        .entry(backlight.serial.clone())
                        .or_insert_with(|| {
                            SliderControl::new(
                                PERCENT,
                                HoverTarget::Backlight(backlight.serial.clone()),
                                backlight.face(),
                            )
                            .scroll_step(STEP)
                            .readout(|value| format!("{}%", as_percent(value)))
                        })
                        .update(ControlMessage::Reading(percent_f(backlight.percent)));
                }
                self.backlights = list;
                Task::none()
            }
            MonitorMessage::Backlight(serial, m) => self.update_backlight(serial, m),
            MonitorMessage::Gamma(reading) => {
                self.gamma.load(reading);
                if let Some(gamma) = self.gamma.get() {
                    self.dim
                        .update(ControlMessage::Reading(percent_f(gamma.brightness)));
                    self.warmth
                        .update(ControlMessage::Reading(f32::from(gamma.temperature)));
                }
                Task::none()
            }
            MonitorMessage::Dim(m) => match self.dim.update(m) {
                Action::Opened => {
                    self.close_backlight_sliders(None);
                    self.warmth.close();
                    Task::none()
                }
                Action::Commit(value) => {
                    let target = as_percent(value);
                    blocking("gamma dim", move || gamma::set_brightness(target))
                        .map(move |ok| MonitorMessage::Dim(ControlMessage::Confirmed { value, ok }))
                }
                Action::None | Action::Right => Task::none(),
            },
            MonitorMessage::Warmth(m) => match self.warmth.update(m) {
                Action::Opened => {
                    self.close_backlight_sliders(None);
                    self.dim.close();
                    Task::none()
                }
                Action::Commit(value) => {
                    let kelvin = as_kelvin(value);
                    blocking("gamma temperature", move || gamma::set_temperature(kelvin)).map(
                        move |ok| MonitorMessage::Warmth(ControlMessage::Confirmed { value, ok }),
                    )
                }
                Action::None | Action::Right => Task::none(),
            },
            MonitorMessage::Clight(m) => self.clight.update(m).map(MonitorMessage::Clight),
            MonitorMessage::Umbra(m) => self.umbra.update(m).map(MonitorMessage::Umbra),
            MonitorMessage::Tooltip(_) => Task::none(),
        }
    }

    fn update_backlight(&mut self, serial: String, m: ControlMessage) -> Task<MonitorMessage> {
        let action = match self.sliders.get_mut(&serial) {
            Some(control) => control.update(m),
            None => return Task::none(),
        };
        match action {
            Action::Opened => {
                self.close_backlight_sliders(Some(&serial));
                self.dim.close();
                self.warmth.close();
                Task::none()
            }
            Action::Commit(value) => {
                let target = as_percent(value);
                let set_serial = serial.clone();
                blocking("backlight set", move || backlight::set(&set_serial, target)).map(
                    move |ok| {
                        MonitorMessage::Backlight(
                            serial.clone(),
                            ControlMessage::Confirmed { value, ok },
                        )
                    },
                )
            }
            Action::None | Action::Right => Task::none(),
        }
    }

    fn close_backlight_sliders(&mut self, keep: Option<&str>) {
        for (serial, control) in &mut self.sliders {
            if Some(serial.as_str()) != keep {
                control.close();
            }
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Backlight(serial) => {
                let backlight = self.backlights.iter().find(|b| &b.serial == serial)?;
                Some(Tooltip {
                    body: backlight.tooltip(),
                    footer: None,
                })
            }
            HoverTarget::GammaDim => self.gamma.get().and_then(|gamma| {
                self.gamma.tooltip_with(TooltipBody::Table(vec![row(
                    "Dim",
                    format!("{}%", gamma.brightness),
                )]))
            }),
            HoverTarget::ColorTemperature => self.gamma.get().and_then(|gamma| {
                self.gamma.tooltip_with(TooltipBody::Table(vec![row(
                    "Temperature",
                    format!("{} K", gamma.temperature),
                )]))
            }),
            HoverTarget::ClightAuto => self.clight.cell().tooltip(),
            HoverTarget::DarkMode => self.umbra.cell().tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, MonitorMessage>> {
        let mut children: Vec<Element<'a, MonitorMessage>> = Vec::new();

        for backlight in &self.backlights {
            if let Some(control) = self.sliders.get(&backlight.serial) {
                children.push(backlight_slider(config, control, &backlight.serial));
            }
        }

        if self.gamma.get().is_some() {
            children.push(self.dim_slider(config));
            children.push(self.warmth_slider(config));
        }

        if let Some(clight) = self.clight.view(config, HoverTarget::ClightAuto, 1.0) {
            children.push(clight.map(lift(MonitorMessage::Clight)));
        }
        if let Some(umbra) = self.umbra.view(config, HoverTarget::DarkMode, 1.0) {
            children.push(umbra.map(lift(MonitorMessage::Umbra)));
        }

        if children.is_empty() {
            return None;
        }

        let content = Row::with_children(children)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
        let reveal = self.menu.reveal();
        Some(self.menu.view(
            config,
            position,
            reveal,
            Self::handle(config),
            content.into(),
            MonitorMessage::Menu,
        ))
    }

    fn handle<'a>(config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        mouse_area(
            Solid::Lightbulb
                .widget()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(MonitorMessage::Menu(MenuMessage::Toggle))
        .into()
    }

    fn dim_slider<'a>(&self, config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        self.dim.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
            m => MonitorMessage::Dim(m),
        })
    }

    fn warmth_slider<'a>(&self, config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        self.warmth.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
            m => MonitorMessage::Warmth(m),
        })
    }
}

fn backlight_slider<'a>(
    config: &ResolvedConfig,
    control: &SliderControl,
    serial: &str,
) -> Element<'a, MonitorMessage> {
    let serial = serial.to_string();
    control.view(config, 1.0).map(move |m| match m {
        ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
        m => MonitorMessage::Backlight(serial.clone(), m),
    })
}

#[allow(clippy::cast_precision_loss)]
fn percent_f(percent: i64) -> f32 {
    percent as f32
}

#[allow(clippy::cast_possible_truncation)]
fn as_percent(value: f32) -> i64 {
    value.round() as i64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}

fn lift<S>(
    wrap: impl Fn(ToggleMessage<S>) -> MonitorMessage,
) -> impl Fn(ToggleMessage<S>) -> MonitorMessage {
    move |m| match m {
        ToggleMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
        m => wrap(m),
    }
}
