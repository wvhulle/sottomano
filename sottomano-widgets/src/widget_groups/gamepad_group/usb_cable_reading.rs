use chrono::Locale;
use sottomano_config::severity::Severity;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
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

impl BarWidget for UsbCableReading {
    fn name(&self) -> &'static str {
        "USB cable"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Usb(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.role {
            UsbCableRole::InputAndCharging => None,
            UsbCableRole::ChargingOnly => Some("charging only".to_owned()),
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.role {
            UsbCableRole::InputAndCharging => WidgetState::Idle,
            UsbCableRole::ChargingOnly => WidgetState::Warning,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        match self.role {
            UsbCableRole::InputAndCharging => {
                TooltipBody::sections([Section::dictionary([KeyValue::new(
                    "Carries",
                    "input and charging",
                )])])
            }
            UsbCableRole::ChargingOnly => TooltipBody::sections([
                Section::dictionary([
                    KeyValue::new("Carries", "charging only").level(Severity::Warning)
                ]),
                Section::note(
                    "Input still goes over Bluetooth",
                    Some(
                        "The controller was already connected over Bluetooth when the cable was \
                         plugged in, so the kernel ignored the cable. Press to disconnect \
                         Bluetooth, then plug the cable in again."
                            .to_owned(),
                    ),
                ),
            ]),
        }
    }
}
