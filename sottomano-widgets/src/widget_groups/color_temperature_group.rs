mod gamma_dim_gauge;
mod sun_follow_toggle;

use std::ops::RangeInclusive;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::screen_brightness_color::gammarelay_color_writes;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    screen_brightness_color::gammarelay_color_temperature::ColorTemperature,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope, unit_active_state::UnitChange},
};
use sottomano_toml_config::Config;
use sun_follow_toggle::SUN_FOLLOW;
use uom::si::thermodynamic_temperature::kelvin;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        unit_switch::{ServiceReadiness, UnitSwitch, UnitSwitchMessage},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
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
    gamma: LatestSnapshot<ColorTemperature>,
    dim: SliderControl,
    warmth: SliderControl,
    auto: UnitSwitch,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ColorTemperatureMessage {
    ColorTemperature(SnapshotDelivery<ColorTemperature>),
    Auto(UnitSwitchMessage),
    Interaction(ColorTemperatureInteraction),
}

#[derive(Debug, Clone)]
pub enum ColorTemperatureInteraction {
    Expansion(WidgetGroupExpansionMessage),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    AutoPressed,
}

impl ColorTemperatureGroup {
    #[must_use]
    pub fn new(unit: String) -> Self {
        Self {
            gamma: LatestSnapshot::default(),
            dim: SliderControl::new(
                "Screen dimming",
                PERCENT,
                ColorTemperatureTarget::GammaDim,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .scroll_step(DIM_STEP)
            .readout(|value, _| format!("{}%", as_percent(value))),
            warmth: SliderControl::new(
                "Color temperature",
                WARMTH_RANGE,
                ColorTemperatureTarget::Temperature,
                Phosphor::ThermometerSimple(Weight::Regular),
            )
            .scroll_step(WARMTH_STEP)
            .slider_step(WARMTH_STEP)
            .readout(|value, _| format!("{}K", as_kelvin(value)))
            .label_with_gauge(|value, _| Some(format!("{}K", as_kelvin(value)))),
            auto: UnitSwitch::new(
                ScopedSystemdUnit {
                    scope: SystemdUnitScope::UserSession,
                    unit_name: unit,
                },
                SUN_FOLLOW,
            ),
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<ColorTemperatureMessage> {
        Subscription::batch([
            snapshot_subscription::<ColorTemperature>(buses.clone())
                .map(ColorTemperatureMessage::ColorTemperature),
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
                first_delivery::<ColorTemperature>(buses.clone())
                    .map(ColorTemperatureMessage::ColorTemperature)
            }
        }
    }

    fn close_controls(&mut self) {
        self.dim.close();
        self.warmth.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_controls();
        }
    }

    pub fn update(
        &mut self,
        message: ColorTemperatureMessage,
        clock: Clock,
    ) -> Task<ColorTemperatureMessage> {
        match message {
            ColorTemperatureMessage::ColorTemperature(reading) => {
                self.gamma.receive(reading, clock);
                if let Some(gamma) = self.gamma.get() {
                    self.dim
                        .update(ControlMessage::Reading(percent_f(gamma.brightness)));
                    self.warmth.update(ControlMessage::Reading(
                        gamma.temperature.get::<kelvin>() as f32
                    ));
                }
                Task::none()
            }
            ColorTemperatureMessage::Auto(unit_switch_message) => self
                .auto
                .update(unit_switch_message, clock)
                .map(ColorTemperatureMessage::Auto),
            ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::Expansion(
                WidgetGroupExpansionMessage::HandleScrolled(delta),
            )) => self.update(
                ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::Warmth(
                    ControlMessage::Scroll(delta),
                )),
                clock,
            ),
            ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::Expansion(
                expansion_message,
            )) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.close_controls();
                }
                Task::none()
            }
            ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::Dim(
                control_message,
            )) => match self.dim.update(control_message) {
                Action::Opened => {
                    self.warmth.close();
                    Task::none()
                }
                Action::Commit(value) => ControlMessage::commit(
                    "gamma dim",
                    value,
                    gammarelay_color_writes::set_brightness(as_percent(value)),
                )
                .map(|control_message| {
                    ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::Dim(
                        control_message,
                    ))
                }),
                Action::None | Action::Right | Action::Adjust(_) => Task::none(),
            },
            ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::Warmth(
                control_message,
            )) => match self.warmth.update(control_message) {
                Action::Opened => {
                    self.dim.close();
                    Task::none()
                }
                Action::Commit(value) => self
                    .auto
                    .request(UnitChange::Stop)
                    .map(ColorTemperatureMessage::Auto)
                    .chain(
                        ControlMessage::commit(
                            "gamma temperature",
                            value,
                            gammarelay_color_writes::set_temperature(as_kelvin(value)),
                        )
                        .map(|control_message| {
                            ColorTemperatureMessage::Interaction(
                                ColorTemperatureInteraction::Warmth(control_message),
                            )
                        }),
                    ),
                Action::None | Action::Right | Action::Adjust(_) => Task::none(),
            },
            ColorTemperatureMessage::Interaction(ColorTemperatureInteraction::AutoPressed) => {
                self.auto.press().map(ColorTemperatureMessage::Auto)
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: ColorTemperatureTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        match target {
            ColorTemperatureTarget::GammaDim => {
                self.gamma
                    .hover_card_with(HoverCardBody::dictionary([LabeledValue::new(
                        "Dim",
                        self.dim.readout_cell(),
                    )]))
            }
            ColorTemperatureTarget::Temperature => {
                self.gamma
                    .hover_card_with(HoverCardBody::dictionary([LabeledValue::new(
                        "Temperature",
                        self.warmth.readout_cell(),
                    )]))
            }
            ColorTemperatureTarget::Automatic => {
                self.auto.hover_card(ServiceReadiness::Serving, clock)
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, ColorTemperatureInteraction> {
        let gamma_available = self.gamma.get().is_some();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::ColorTemperature,
            &self.expansion,
            ColorTemperatureInteraction::Expansion,
            [
                gamma_available.then(|| {
                    GroupItem::slider(&self.warmth, config, ColorTemperatureInteraction::Warmth)
                        .previewed_on_handle()
                }),
                gamma_available.then(|| {
                    GroupItem::slider(&self.dim, config, ColorTemperatureInteraction::Dim)
                }),
                self.auto.group_item(
                    ServiceReadiness::Serving,
                    ColorTemperatureTarget::Automatic,
                    ColorTemperatureInteraction::AutoPressed,
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::ThermometerHot(Weight::Regular).into(),
        })
        .forwards_scroll(gamma_available.then_some("change the screen warmth"))
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
