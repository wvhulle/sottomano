use chrono::Locale;
use sottomano_desktop_sources::gamepad_controller_input::evdev_gamepad_devices::GamepadPower;
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const LOW_BATTERY_PERCENT: u8 = 15;

pub struct ControllerBatteryReading {
    pub power: GamepadPower,
}

impl ControllerBatteryReading {
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

impl GroupItemReading for ControllerBatteryReading {
    fn name(&self) -> &'static str {
        "Controller battery"
    }

    fn icon(&self) -> IconGlyph {
        match self.power {
            GamepadPower::Charging(_) => Phosphor::BatteryCharging(Weight::Regular).into(),
            GamepadPower::Full => Phosphor::BatteryFull(Weight::Regular).into(),
            GamepadPower::Discharging(percent) if percent <= LOW_BATTERY_PERCENT => {
                Phosphor::BatteryWarning(Weight::Regular).into()
            }
            GamepadPower::Discharging(percent) if percent <= 50 => {
                Phosphor::BatteryMedium(Weight::Regular).into()
            }
            GamepadPower::Discharging(_) | GamepadPower::NoBattery => {
                Phosphor::BatteryHigh(Weight::Regular).into()
            }
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.power {
            GamepadPower::Discharging(percent) | GamepadPower::Charging(percent) => {
                Some(format!("{percent}%"))
            }
            GamepadPower::Full => Some("full".to_owned()),
            GamepadPower::NoBattery => None,
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.battery_level() == BatteryLevel::Low {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Battery", self.charge_description()).level(
                if self.battery_level() == BatteryLevel::Low {
                    Severity::Warning
                } else {
                    Severity::Normal
                },
            ),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryLevel {
    Low,
    Comfortable,
}
