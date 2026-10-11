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
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const DIM_STEP: f32 = 5.0;
const WARMTH_RANGE: RangeInclusive<f32> = 3000.0..=6500.0;
const WARMTH_STEP: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTemperatureTarget {
    GammaDim,
    Temperature,
    Automatic,
}

impl From<ColorTemperatureTarget> for HoverTarget {
    fn from(target: ColorTemperatureTarget) -> Self {
        HoverTarget::ColorTemperature(target)
    }
}

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
                "Screen dimming",
                PERCENT,
                ColorTemperatureTarget::GammaDim,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .scroll_step(DIM_STEP)
            .readout(|value| format!("{}%", as_percent(value))),
            warmth: SliderControl::new(
                "Color temperature",
                WARMTH_RANGE,
                ColorTemperatureTarget::Temperature,
                Phosphor::ThermometerSimple(Weight::Regular),
            )
            .scroll_step(WARMTH_STEP)
            .slider_step(WARMTH_STEP)
            .readout(|value| format!("{}K", as_kelvin(value)))
            .label_with_gauge(|value| Some(format!("{}K", as_kelvin(value)))),
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
            ColorTemperatureMessage::Expansion(ExpansionMessage::HandleScrolled(delta)) => self
                .update(ColorTemperatureMessage::Warmth(ControlMessage::Scroll(
                    delta,
                ))),
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

    pub fn tooltip_for(&self, target: ColorTemperatureTarget) -> Option<Tooltip> {
        match target {
            ColorTemperatureTarget::GammaDim => {
                self.gamma
                    .tooltip_with(TooltipBody::sections(&[Section::untitled_dictionary([
                        KeyValue::new("Dim", self.dim.readout_text()),
                    ])]))
            }
            ColorTemperatureTarget::Temperature => {
                self.gamma
                    .tooltip_with(TooltipBody::sections(&[Section::untitled_dictionary([
                        KeyValue::new("Temperature", self.warmth.readout_text()),
                    ])]))
            }
            ColorTemperatureTarget::Automatic => self.auto.tooltip(),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, ColorTemperatureMessage> {
        let gamma_available = self.gamma.get().is_some();
        GroupContent::new(
            &self.expansion,
            ColorTemperatureMessage::Expansion,
            [
                gamma_available.then(|| {
                    GroupItem::slider(&self.warmth, config, ColorTemperatureMessage::Warmth)
                }),
                gamma_available
                    .then(|| GroupItem::slider(&self.dim, config, ColorTemperatureMessage::Dim)),
                self.auto.group_item(
                    ColorTemperatureTarget::Automatic,
                    ColorTemperatureMessage::Auto,
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon(
            Phosphor::ThermometerHot(Weight::Regular).into(),
        ))
        .preview(gamma_available.then_some(PreviewedValue::Plain(&self.warmth as &dyn BarWidget)))
        .forwards_scroll(gamma_available)
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
