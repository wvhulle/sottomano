use crate::group_indicators::{
    indicator_group_content::IndicatorListing, indicator_state::IndicatorState,
};
mod dim_indicator;
mod sun_follow_indicator;
mod warmth_indicator;

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
use sun_follow_indicator::SUN_FOLLOW;
use uom::si::thermodynamic_temperature::kelvin;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            ExpansionChange, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        unit_switch::{UnitSwitch, UnitSwitchMessage},
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
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
    expansion: IndicatorGroupExpansion,
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
    Expansion(IndicatorGroupExpansionMessage),
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
            expansion: IndicatorGroupExpansion::default(),
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
            ColorTemperatureMessage::ColorTemperature(sample) => {
                self.gamma.receive(sample, clock);
                if let Some(gamma) = self.gamma.get() {
                    self.dim
                        .update(ControlMessage::Sample(percent_f(gamma.brightness)));
                    self.warmth.update(ControlMessage::Sample(
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
                IndicatorGroupExpansionMessage::HandleScrolled(delta),
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

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let gamma_available = self.gamma.get().is_some();
        let rows: Vec<_> = self
            .warmth_rows(config)
            .into_iter()
            .chain(self.dim_rows(config))
            .chain(self.sun_follow_rows())
            .collect();
        let warmth_content = |headline_indicator| {
            IndicatorGroupContent::expandable(
                IndicatorGroupFamily::ColorTemperature,
                headline_indicator,
                &self.expansion,
                ColorTemperatureInteraction::Expansion,
                IndicatorListing::Listed(rows),
            )
        };
        if gamma_available {
            warmth_content(
                self.warmth
                    .indicator(config.calendar.locale)
                    .with_headline_glyph(Phosphor::ThermometerHot(Weight::Regular).into()),
            )
            .forwards_scroll("change the screen warmth")
        } else {
            warmth_content(GroupIndicator::glyph_only(
                IndicatorGroupFamily::ColorTemperature.title(),
                Phosphor::ThermometerHot(Weight::Regular).into(),
                IndicatorState::Idle,
            ))
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}
