use crate::group_items::reading_display_state::ReadingDisplayState;
mod sun_follow_toggle;

use std::ops::RangeInclusive;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::screen_brightness_color::gammarelay_color_writes;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_run_state::RunChange,
    screen_brightness_color::gammarelay_color_temperature::ColorTemperature,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
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
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
const DIM_STEP: f32 = 5.0;
const WARMTH_RANGE: RangeInclusive<f32> = 3000.0..=6500.0;
const WARMTH_STEP: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTemperatureBusRequest {
    Refresh,
}

pub struct ColorTemperatureGroup {
    gamma: SubscribedSnapshot<ColorTemperature>,
    dim: SliderControl,
    warmth: SliderControl,
    auto: UnitSwitch,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ColorTemperatureMessage {
    ColorTemperature(SnapshotDelivery<ColorTemperature>),
    Auto(UnitSwitchMessage),
    DimCommitted(ControlMessage),
    WarmthCommitted(ControlMessage),
}

#[derive(Debug, Clone)]
pub enum ColorTemperatureInteraction {
    Expansion(WidgetGroupExpansionMessage),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    AutoPressed,
    OnBuses(ColorTemperatureBusRequest),
}

impl ColorTemperatureGroup {
    #[must_use]
    pub fn new(unit: String) -> Self {
        Self {
            gamma: SubscribedSnapshot::default(),
            dim: SliderControl::new(
                "Screen dimming",
                PERCENT,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .scroll_step(DIM_STEP)
            .readout(|value, _| format!("{}%", as_percent(value))),
            warmth: SliderControl::new(
                "Warmth",
                WARMTH_RANGE,
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

    pub fn on_buses(
        request: ColorTemperatureBusRequest,
        buses: &DesktopBuses,
    ) -> Task<ColorTemperatureMessage> {
        match request {
            ColorTemperatureBusRequest::Refresh => {
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
            ColorTemperatureMessage::DimCommitted(control_message) => {
                self.control_dim(control_message)
            }
            ColorTemperatureMessage::WarmthCommitted(control_message) => {
                self.control_warmth(control_message)
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: ColorTemperatureInteraction,
    ) -> Task<ColorTemperatureMessage> {
        match interaction {
            ColorTemperatureInteraction::Expansion(
                WidgetGroupExpansionMessage::HandleScrolled(delta),
            ) => self.control_warmth(ControlMessage::Scroll(delta)),
            ColorTemperatureInteraction::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.close_controls();
                }
                Task::none()
            }
            ColorTemperatureInteraction::Dim(control_message) => self.control_dim(control_message),
            ColorTemperatureInteraction::Warmth(control_message) => {
                self.control_warmth(control_message)
            }
            ColorTemperatureInteraction::AutoPressed => {
                self.auto.press().map(ColorTemperatureMessage::Auto)
            }
            ColorTemperatureInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn control_dim(&mut self, control_message: ControlMessage) -> Task<ColorTemperatureMessage> {
        match self.dim.update(control_message) {
            Action::Opened => {
                self.warmth.close();
                Task::none()
            }
            Action::Commit(value) => ControlMessage::commit(
                "gamma dim",
                value,
                gammarelay_color_writes::set_brightness(as_percent(value)),
            )
            .map(ColorTemperatureMessage::DimCommitted),
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn control_warmth(&mut self, control_message: ControlMessage) -> Task<ColorTemperatureMessage> {
        match self.warmth.update(control_message) {
            Action::Opened => {
                self.dim.close();
                Task::none()
            }
            Action::Commit(value) => self
                .auto
                .request(RunChange::Stop)
                .map(ColorTemperatureMessage::Auto)
                .chain(
                    ControlMessage::commit(
                        "gamma temperature",
                        value,
                        gammarelay_color_writes::set_temperature(as_kelvin(value)),
                    )
                    .map(ColorTemperatureMessage::WarmthCommitted),
                ),
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, ColorTemperatureInteraction> {
        let gamma_available = self.gamma.get().is_some();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::ColorTemperature,
            GroupSummary::glyph(
                Phosphor::ThermometerHot(Weight::Regular).into(),
                ReadingDisplayState::Idle,
            ),
            &self.expansion,
            ColorTemperatureInteraction::Expansion,
            [
                gamma_available.then(|| {
                    GroupItem::slider(&self.warmth, config, ColorTemperatureInteraction::Warmth)
                        .with_hover_card(Box::new(move |_| {
                            self.gamma.hover_card_with(vec![Section::new(
                                "Setting",
                                dictionary_table([LabeledValue::new(
                                    "Temperature",
                                    self.warmth.readout_cell(),
                                )]),
                            )])
                        }))
                        .refreshed_by(ColorTemperatureInteraction::OnBuses(
                            ColorTemperatureBusRequest::Refresh,
                        ))
                }),
                gamma_available.then(|| {
                    GroupItem::slider(&self.dim, config, ColorTemperatureInteraction::Dim)
                        .with_hover_card(Box::new(move |_| {
                            self.gamma.hover_card_with(vec![Section::new(
                                "Setting",
                                dictionary_table([LabeledValue::new(
                                    "Dim",
                                    self.dim.readout_cell(),
                                )]),
                            )])
                        }))
                        .refreshed_by(ColorTemperatureInteraction::OnBuses(
                            ColorTemperatureBusRequest::Refresh,
                        ))
                }),
                self.auto
                    .group_item(
                        ServiceReadiness::Serving,
                        ColorTemperatureInteraction::AutoPressed,
                    )
                    .map(|auto_item| {
                        auto_item
                            .with_hover_card(Box::new(move |_| {
                                self.auto.hover_card(ServiceReadiness::Serving)
                            }))
                            .refreshed_by(ColorTemperatureInteraction::OnBuses(
                                ColorTemperatureBusRequest::Refresh,
                            ))
                    }),
            ]
            .into_iter()
            .flatten(),
        )
        .forwards_scroll(gamma_available.then_some("change the screen warmth"))
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
