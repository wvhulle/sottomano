use sottomano_desktop_sources::gamepad_controller_input::evdev_gamepad_devices::{
    ConnectedGamepad, GamepadPower, GamepadTransport, PhysicalGamepadLink,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const LOW_BATTERY_PERCENT: u8 = 15;

pub struct ControllerBatteryIndicator {
    pub power: GamepadPower,
}

impl ControllerBatteryIndicator {
    pub(super) fn battery_level(&self) -> BatteryLevel {
        if matches!(self.power, GamepadPower::Discharging(percent) if percent <= LOW_BATTERY_PERCENT)
        {
            BatteryLevel::Low
        } else {
            BatteryLevel::Comfortable
        }
    }

    fn charge_description(&self) -> String {
        match self.power {
            GamepadPower::NoBattery => "no battery reported".to_owned(),
            GamepadPower::Discharging(percent) => format!("{percent}%"),
            GamepadPower::Charging(percent) => format!("{percent}%, charging"),
            GamepadPower::Full => "full".to_owned(),
        }
    }
}

impl ControllerBatteryIndicator {
    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = match self.power {
            GamepadPower::Charging(_) => Phosphor::BatteryCharging(Weight::Regular),
            GamepadPower::Full => Phosphor::BatteryFull(Weight::Regular),
            GamepadPower::Discharging(percent) if percent <= LOW_BATTERY_PERCENT => {
                Phosphor::BatteryWarning(Weight::Regular)
            }
            GamepadPower::Discharging(percent) if percent <= 50 => {
                Phosphor::BatteryMedium(Weight::Regular)
            }
            GamepadPower::Discharging(_) | GamepadPower::NoBattery => {
                Phosphor::BatteryHigh(Weight::Regular)
            }
        };
        let label = match self.power {
            GamepadPower::Discharging(percent) | GamepadPower::Charging(percent) => {
                ReadoutLabel::Labeled(format!("{percent}%"))
            }
            GamepadPower::Full => ReadoutLabel::Labeled("full".to_owned()),
            GamepadPower::NoBattery => ReadoutLabel::Unlabeled,
        };
        let state = if self.battery_level() == BatteryLevel::Low {
            IndicatorState::Warning
        } else {
            IndicatorState::Idle
        };
        GroupIndicator::glyph_only("Controller battery", glyph.into(), state)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Charge",
            dictionary_table([
                LabeledValue::new("Battery", self.charge_description()).level(
                    if self.battery_level() == BatteryLevel::Low {
                        Severity::Warning
                    } else {
                        Severity::Normal
                    },
                ),
            ]),
        )]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryLevel {
    Low,
    Comfortable,
}

impl ControllerBatteryIndicator {
    pub(super) fn battery_rows<'view>(
        gamepad: &ConnectedGamepad,
        link: &PhysicalGamepadLink,
    ) -> Vec<GroupIndicator<'view>> {
        if matches!(link.transport, GamepadTransport::Bluetooth { .. })
            || gamepad.power == GamepadPower::NoBattery
        {
            return Vec::new();
        }
        vec![
            ControllerBatteryIndicator {
                power: gamepad.power,
            }
            .indicator(),
        ]
    }
}
