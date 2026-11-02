use chrono::Locale;
use sottomano_config::severity::Severity;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbCableRole {
    InputAndCharging,
    ChargingOnly,
}

pub struct UsbCableReading {
    pub role: UsbCableRole,
}

impl GroupItemReading for UsbCableReading {
    fn name(&self) -> &'static str {
        "USB cable"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Usb(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.role {
            UsbCableRole::InputAndCharging => None,
            UsbCableRole::ChargingOnly => Some("charging only".to_owned()),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.role {
            UsbCableRole::InputAndCharging => ReadingDisplayState::Idle,
            UsbCableRole::ChargingOnly => ReadingDisplayState::Warning,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        match self.role {
            UsbCableRole::InputAndCharging => {
                TooltipBody::sections([Section::dictionary([LabeledValue::new(
                    "Carries",
                    "input and charging",
                )])])
            }
            UsbCableRole::ChargingOnly => TooltipBody::sections([
                Section::dictionary([
                    LabeledValue::new("Carries", "charging only").level(Severity::Warning)
                ]),
                Section::detailed_note(
                    "Input still goes over Bluetooth",
                    "The controller was already connected over Bluetooth when the cable was \
                     plugged in, so the kernel ignored the cable. Press to disconnect Bluetooth, \
                     then plug the cable in again."
                        .to_owned(),
                ),
            ]),
        }
    }
}
