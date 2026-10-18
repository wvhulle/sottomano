mod gamma_dim_gauge;
mod sun_follow_toggle;

use std::ops::RangeInclusive;

use iced::{Subscription, Task};
use sun_follow_toggle::SUN_FOLLOW;
use uom::si::thermodynamic_temperature::kelvin;
use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    bus::DesktopBuses,
    gamma::{self, Gamma},
    systemd_manager::{ScopedSystemdUnit, SystemdUnitScope, unit_activity::UnitChange},
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        unit_switch::{UnitSwitch, UnitSwitchMessage},
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
    gamma: ContinuousReading<Gamma>,
    dim: SliderControl,
    warmth: SliderControl,
    auto: UnitSwitch,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ColorTemperatureMessage {
    Expansion(ExpansionMessage),
    Gamma(SourceDelivery<Gamma>),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    Auto(UnitSwitchMessage),
}

impl ColorTemperatureGroup {
    #[must_use]
    pub fn new(unit: String) -> Self {
        Self {
            gamma: ContinuousReading::default(),
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
            auto: UnitSwitch::new(
                ScopedSystemdUnit {
                    scope: SystemdUnitScope::UserSession,
                    unit_name: unit,
                },
                SUN_FOLLOW,
            ),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<ColorTemperatureMessage> {
        Subscription::batch([
            source_subscription::<Gamma>(buses.clone()).map(ColorTemperatureMessage::Gamma),
            self.auto
                .subscription(buses)
                .map(ColorTemperatureMessage::Auto),
        ])
    }

    pub fn refresh(
        target: ColorTemperatureTarget,
        buses: &DesktopBuses,
    ) -> Task<ColorTemperatureMessage> {
        match target {
            ColorTemperatureTarget::GammaDim
            | ColorTemperatureTarget::Temperature
            | ColorTemperatureTarget::Automatic => {
                first_delivery::<Gamma>(buses.clone()).map(ColorTemperatureMessage::Gamma)
            }
        }
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
                self.gamma.receive_delivery(reading);
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
                Action::Commit(value) => self
                    .auto
                    .request(UnitChange::Stop)
                    .map(ColorTemperatureMessage::Auto)
                    .chain(
                        ControlMessage::commit("gamma temperature", value, move || {
                            gamma::set_temperature(as_kelvin(value))
                        })
                        .map(ColorTemperatureMessage::Warmth),
                    ),
                Action::None | Action::Right | Action::Adjust(_) => Task::none(),
            },
            ColorTemperatureMessage::Auto(m) => {
                self.auto.update(m).map(ColorTemperatureMessage::Auto)
            }
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: ColorTemperatureTarget) -> Option<Tooltip> {
        match target {
            ColorTemperatureTarget::GammaDim => {
                self.gamma
                    .tooltip_with(TooltipBody::dictionary([KeyValue::new(
                        "Dim",
                        self.dim.readout_text(),
                    )]))
            }
            ColorTemperatureTarget::Temperature => {
                self.gamma
                    .tooltip_with(TooltipBody::dictionary([KeyValue::new(
                        "Temperature",
                        self.warmth.readout_text(),
                    )]))
            }
            ColorTemperatureTarget::Automatic => self.auto.tooltip(true),
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
                    true,
                    ColorTemperatureTarget::Automatic,
                    ColorTemperatureMessage::Auto(UnitSwitchMessage::Press),
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::ThermometerHot(Weight::Regular).into(),
            handle: GroupHandle::ColorTemperature,
        })
        .preview(gamma_available.then_some(PreviewedValue::Plain(&self.warmth as &dyn BarWidget)))
        .forwards_scroll(gamma_available.then_some("change the screen warmth"))
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
