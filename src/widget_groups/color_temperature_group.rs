pub mod gamma_dim_gauge;
pub mod sun_follow_toggle;

use std::ops::RangeInclusive;

use iced::{Alignment, Element, Subscription, Task, widget::Row};
use uom::si::thermodynamic_temperature::kelvin;

use crate::{
    config::Config,
    data::source::{
        Source, bus,
        gamma::{self, Gamma},
        screen_temperature::{self, SunFollow},
    },
    message_subscriptions::polling,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        bar_widget::Glyph,
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::{Tooltip, TooltipBody, row},
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const DIM_STEP: f32 = 5.0;
const WARMTH_RANGE: RangeInclusive<f32> = 3000.0..=6500.0;
const WARMTH_STEP: f32 = 100.0;

pub struct ColorTemperatureGroup {
    unit: String,
    gamma: LastPoll<Gamma>,
    dim: SliderControl,
    warmth: SliderControl,
    auto: ToggleControl<SunFollow>,
    relay: bool,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum ColorTemperatureMessage {
    Menu(MenuMessage),
    Relay(bool),
    Gamma(Result<Gamma, bus::Error>),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    Auto(ToggleMessage<SunFollow>),
    Tooltip(TooltipMessage),
}

impl ColorTemperatureGroup {
    pub fn new(unit: String) -> Self {
        Self {
            unit,
            gamma: LastPoll::default(),
            dim: SliderControl::new(
                PERCENT,
                HoverTarget::GammaDim,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .scroll_step(DIM_STEP)
            .readout(|value| format!("{}%", as_percent(value))),
            warmth: SliderControl::new(
                WARMTH_RANGE,
                HoverTarget::ColorTemperature,
                Phosphor::ThermometerSimple(Weight::Regular),
            )
            .scroll_step(WARMTH_STEP)
            .slider_step(WARMTH_STEP)
            .readout_width(52.0)
            .readout(|value| format!("{}K", as_kelvin(value)))
            .label(|value| format!("{}K", as_kelvin(value))),
            auto: ToggleControl::default(),
            relay: false,
            menu: MenuGroup::default(),
        }
    }

    pub fn probe(&self) -> Task<ColorTemperatureMessage> {
        Task::batch([
            Task::future(async { ColorTemperatureMessage::Relay(Gamma::available(&()).await) }),
            ToggleControl::<SunFollow>::probe(self.unit.clone()).map(ColorTemperatureMessage::Auto),
        ])
    }

    pub fn subscription(&self) -> Subscription<ColorTemperatureMessage> {
        let gamma = if self.relay {
            polling::<Gamma>(()).map(ColorTemperatureMessage::Gamma)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            gamma,
            self.auto
                .subscription(self.unit.clone())
                .map(ColorTemperatureMessage::Auto),
        ])
    }

    pub fn is_animating(&self) -> bool {
        self.dim.is_animating() || self.warmth.is_animating()
    }

    fn close_controls(&mut self) {
        self.dim.close();
        self.warmth.close();
    }

    pub fn close_menu(&mut self) {
        if self.menu.close() {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: ColorTemperatureMessage) -> Task<ColorTemperatureMessage> {
        match msg {
            ColorTemperatureMessage::Relay(relay) => {
                self.relay = relay;
                Task::none()
            }
            ColorTemperatureMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.close_controls();
                }
                Task::none()
            }
            ColorTemperatureMessage::Gamma(reading) => {
                self.gamma.receive(reading);
                if let Some(gamma) = self.gamma.get() {
                    self.dim
                        .update(ControlMessage::Reading(percent_f(gamma.brightness)));
                    self.warmth.update(ControlMessage::Reading(
                        gamma.temperature.get::<kelvin>() as f32
                    ));
                }
                Task::none()
            }
            ColorTemperatureMessage::Dim(m) => match self.dim.update(m) {
                Action::Opened => {
                    self.warmth.close();
                    Task::none()
                }
                Action::Commit(value) => ControlMessage::commit("gamma dim", value, move || {
                    gamma::set_brightness(as_percent(value))
                })
                .map(ColorTemperatureMessage::Dim),
                Action::None | Action::Right | Action::Adjust(_) => Task::none(),
            },
            ColorTemperatureMessage::Warmth(m) => match self.warmth.update(m) {
                Action::Opened => {
                    self.dim.close();
                    Task::none()
                }
                Action::Commit(value) => {
                    self.auto.assume(SunFollow {
                        auto: false,
                        unit: self.unit.clone(),
                    });
                    let unit = self.unit.clone();
                    ControlMessage::commit("gamma temperature", value, move || {
                        screen_temperature::set_manual(&unit, as_kelvin(value))
                    })
                    .map(ColorTemperatureMessage::Warmth)
                }
                Action::None | Action::Right | Action::Adjust(_) => Task::none(),
            },
            ColorTemperatureMessage::Auto(m) => {
                self.auto.update(m).map(ColorTemperatureMessage::Auto)
            }
            ColorTemperatureMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::GammaDim => self
                .gamma
                .tooltip_with(TooltipBody::rows(vec![row("Dim", self.dim.readout_text())])),
            HoverTarget::ColorTemperature => self.gamma.tooltip_with(TooltipBody::rows(vec![row(
                "Temperature",
                self.warmth.readout_text(),
            )])),
            HoverTarget::AutoColorTemperature => self.auto.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, ColorTemperatureMessage>> {
        let children = move || {
            let mut children: Vec<Element<'a, ColorTemperatureMessage>> = Vec::new();
            if self.gamma.get().is_some() {
                children.push(self.warmth_slider(config));
                children.push(self.dim_slider(config));
            }
            if let Some(auto) = self
                .auto
                .view(config, HoverTarget::AutoColorTemperature, 1.0)
            {
                children.push(auto.map(|m| match m {
                    ToggleMessage::Tooltip(t) => ColorTemperatureMessage::Tooltip(t),
                    m => ColorTemperatureMessage::Auto(m),
                }));
            }
            children
        };

        if self.gamma.get().is_none() && self.auto.reading().is_none() {
            return None;
        }

        let content = move |_reveal: f32| {
            Row::with_children(children())
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };
        Some(
            self.menu
                .slot(
                    move || Self::face(config),
                    content,
                    ColorTemperatureMessage::Menu,
                )
                .view(config, position),
        )
    }

    fn face<'a>(config: &Config) -> Element<'a, ColorTemperatureMessage> {
        Glyph::from(Phosphor::Thermometer(Weight::Regular))
            .view(config.font.icon_size_px, config.palette.bar_foreground)
    }

    fn dim_slider<'a>(&self, config: &Config) -> Element<'a, ColorTemperatureMessage> {
        self.dim.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => ColorTemperatureMessage::Tooltip(t),
            m => ColorTemperatureMessage::Dim(m),
        })
    }

    fn warmth_slider<'a>(&self, config: &Config) -> Element<'a, ColorTemperatureMessage> {
        self.warmth.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => ColorTemperatureMessage::Tooltip(t),
            m => ColorTemperatureMessage::Warmth(m),
        })
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
