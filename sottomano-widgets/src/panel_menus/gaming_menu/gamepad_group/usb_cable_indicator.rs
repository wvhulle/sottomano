use sottomano_desktop_sources::gamepad_controller_input::evdev_gamepad_devices::{
    GamepadTransport, PhysicalGamepadLink, UsbCableState,
};
use sottomano_toml_config::severity::Severity;

use super::GamepadInteraction;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    widgets::value_cells::LabeledValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbCableRole {
    InputAndCharging,
    ChargingOnly,
}

pub struct UsbCableIndicator {
    pub role: UsbCableRole,
}

impl UsbCableIndicator {
    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let (label, state) = match self.role {
            UsbCableRole::InputAndCharging => (ReadoutLabel::Unlabeled, IndicatorState::Idle),
            UsbCableRole::ChargingOnly => (
                ReadoutLabel::Labeled("charging only".to_owned()),
                IndicatorState::Warning,
            ),
        };
        GroupIndicator::glyph_only("USB cable", Phosphor::Usb(Weight::Regular).into(), state)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        match self.role {
            UsbCableRole::InputAndCharging => {
                vec![Section::new(
                    "Connection",
                    dictionary_table([LabeledValue::new("Carries", "input and charging")]),
                )]
            }
            UsbCableRole::ChargingOnly => vec![
                Section::new(
                    "Connection",
                    dictionary_table([
                        LabeledValue::new("Carries", "charging only").level(Severity::Warning)
                    ]),
                ),
                Section::new(
                    "Input still goes over Bluetooth",
                    SectionWidget::paragraph(
                        "The controller was already connected over Bluetooth when the cable was \
                         plugged in, so the kernel ignored the cable. Press to disconnect \
                         Bluetooth, then plug the cable in again."
                            .to_owned(),
                    ),
                ),
            ],
        }
    }
}

impl UsbCableIndicator {
    pub(super) fn cable_rows(slot: usize, link: &PhysicalGamepadLink) -> Vec<GroupIndicator<'_>> {
        let Some(cable_role) = usb_cable_role(link.transport) else {
            return Vec::new();
        };
        let cable_indicator = UsbCableIndicator { role: cable_role }.indicator();
        vec![match cable_role {
            UsbCableRole::ChargingOnly => cable_indicator.on_press(
                "disconnect Bluetooth so the cable can take over",
                GamepadInteraction::DisconnectBluetooth { slot },
            ),
            UsbCableRole::InputAndCharging => cable_indicator,
        }]
    }
}

fn usb_cable_role(transport: GamepadTransport) -> Option<UsbCableRole> {
    match transport {
        GamepadTransport::Usb => Some(UsbCableRole::InputAndCharging),
        GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::IgnoredByKernel,
        } => Some(UsbCableRole::ChargingOnly),
        GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::Absent,
        }
        | GamepadTransport::SteamDongle
        | GamepadTransport::Other => None,
    }
}
