use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::gamepad_controller_input::evdev_gamepad_devices::{
    ControllerIdentity, GamepadTransport, UsbCableState,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct ControllerTransportReading<'a> {
    pub transport: GamepadTransport,
    pub identity: &'a ControllerIdentity,
}

impl ControllerTransportReading<'_> {
    #[must_use]
    pub const fn cable_ignored(&self) -> bool {
        matches!(
            self.transport,
            GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::IgnoredByKernel
            }
        )
    }

    fn address_entry(&self) -> Option<KeyValue> {
        match self.identity {
            ControllerIdentity::Addressed {
                hardware_address, ..
            } => Some(KeyValue::new("Address", hardware_address.clone())),
            ControllerIdentity::Unaddressed => None,
        }
    }

    fn connection_section(&self) -> Section {
        let transport_entries = match self.transport {
            GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::Absent,
            } => vec![
                KeyValue::new("Connection", "Bluetooth"),
                KeyValue::new("USB cable", "not plugged in"),
            ],
            GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::IgnoredByKernel,
            } => vec![
                KeyValue::new("Connection", "Bluetooth"),
                KeyValue::new("USB cable", "plugged in, only charging").level(Severity::Warning),
            ],
            GamepadTransport::Usb => vec![KeyValue::new("Connection", "USB cable")],
            GamepadTransport::Other => vec![KeyValue::new("Connection", "other bus")],
        };
        Section::dictionary(transport_entries.into_iter().chain(self.address_entry()))
    }

    fn explanation_section(&self) -> Option<Section> {
        match self.transport {
            GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::IgnoredByKernel,
            } => Some(Section::note(
                "Cable ignored by the kernel",
                Some(
                    "The controller was already connected over Bluetooth when the cable was \
                     plugged in, so input still goes over Bluetooth. Press to disconnect \
                     Bluetooth, then plug the cable in again."
                        .to_owned(),
                ),
            )),
            GamepadTransport::Usb => Some(Section::note(
                "Wired",
                Some("Input goes over the cable, which also charges the controller.".to_owned()),
            )),
            GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::Absent,
            }
            | GamepadTransport::Other => None,
        }
    }
}

impl BarWidget for ControllerTransportReading<'_> {
    fn name(&self) -> &'static str {
        "Controller connection"
    }

    fn icon(&self) -> Glyph {
        match self.transport {
            GamepadTransport::Bluetooth { .. } => Phosphor::Bluetooth(Weight::Regular).into(),
            GamepadTransport::Usb => Phosphor::Usb(Weight::Regular).into(),
            GamepadTransport::Other => Phosphor::PlugsConnected(Weight::Regular).into(),
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.cable_ignored().then(|| "cable unused".to_owned())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.cable_ignored() {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections(
            [Some(self.connection_section()), self.explanation_section()]
                .into_iter()
                .flatten(),
        )
    }
}
