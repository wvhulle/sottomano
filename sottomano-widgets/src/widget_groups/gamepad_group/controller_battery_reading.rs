use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::gamepad_controller_input::evdev_gamepad_devices::GamepadPower;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

const LOW_BATTERY_PERCENT: u8 = 15;

pub struct ControllerBatteryReading {
    pub power: GamepadPower,
}

impl ControllerBatteryReading {
    pub(super) const fn running_low(&self) -> bool {
        matches!(self.power, GamepadPower::Discharging(percent) if percent <= LOW_BATTERY_PERCENT)
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

impl BarWidget for ControllerBatteryReading {
    fn name(&self) -> &'static str {
        "Controller battery"
    }

    fn icon(&self) -> Glyph {
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

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.running_low() {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Battery", self.charge_description()).level(if self.running_low() {
                Severity::Warning
            } else {
                Severity::Normal
            }),
        ])
    }
}
