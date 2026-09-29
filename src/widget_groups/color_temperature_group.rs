pub mod gamma_dim_gauge;
pub mod sun_follow_toggle;

use std::ops::RangeInclusive;

use iced::{Subscription, Task};
use uom::si::thermodynamic_temperature::kelvin;

use crate::{
    config::Config,
    data::source::{
        Source, bus,
        gamma::{self, Gamma},
        screen_temperature::{self, SunFollow},
    },
    message_subscriptions::polling,
    wayland_layers::tooltip::HoverTarget,
    widgets::{
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::{Tooltip, TooltipBody, row},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
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
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ColorTemperatureMessage {
    Expansion(ExpansionMessage),
    Relay(bool),
    Gamma(Result<Gamma, bus::Error>),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    Auto(ToggleMessage<SunFollow>),
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
            expansion: GroupExpansion::default(),
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

    fn close_controls(&mut self) {
        self.dim.close();
        self.warmth.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_controls();
        }
    }

    pub fn update(&mut self, msg: ColorTemperatureMessage) -> Task<ColorTemperatureMessage> {
        match msg {
            ColorTemperatureMessage::Relay(relay) => {
                self.relay = relay;
                Task::none()
            }
            ColorTemperatureMessage::Expansion(m) => {
                if self.expansion.update(m) {
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

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroup<'a, ColorTemperatureMessage> {
        let gamma_sliders = self.gamma.get().map(|_| {
            [
                GroupItem::slider(&self.warmth, config, ColorTemperatureMessage::Warmth),
                GroupItem::slider(&self.dim, config, ColorTemperatureMessage::Dim),
            ]
        });
        WidgetGroup::new(&self.expansion, ColorTemperatureMessage::Expansion)
            .icon(Phosphor::Thermometer(Weight::Regular))
            .items(gamma_sliders.into_iter().flatten())
            .item(GroupItem::toggle(
                &self.auto,
                HoverTarget::AutoColorTemperature,
                ColorTemperatureMessage::Auto,
            ))
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
