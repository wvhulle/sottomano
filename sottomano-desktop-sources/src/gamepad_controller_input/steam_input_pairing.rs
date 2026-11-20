use std::collections::BTreeMap;

use super::{
    evdev_gamepad_devices::{
        ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower, GamepadTransport,
        InputRoute, PhysicalGamepadLink, RawGamepadEvent,
    },
    player_indicator_leds::PlayerIndicator,
    steam_dongle_hidraw::{DongleInterface, DongleSample},
};

pub const STEAM_CONTROLLER_NAME: &str = "Steam Controller";

#[derive(Debug, PartialEq)]
pub enum DeviceSignal {
    Raw(RawGamepadEvent),
    Dongle(DongleInterface, DongleSample),
}

#[derive(Debug, Default)]
pub struct SteamInputPairing {
    latest_roster: Option<Vec<ConnectedGamepad>>,
    delivered_roster: Vec<ConnectedGamepad>,
    live_interfaces: BTreeMap<DongleInterface, bool>,
    battery_by_interface: BTreeMap<DongleInterface, GamepadPower>,
}

fn steam_input_controller(
    steam_virtual_pad: &ConnectedGamepad,
    interface: &DongleInterface,
    power: GamepadPower,
) -> ConnectedGamepad {
    ConnectedGamepad {
        slot: steam_virtual_pad.slot,
        name: STEAM_CONTROLLER_NAME.to_owned(),
        vendor_id: steam_virtual_pad.vendor_id,
        product_id: steam_virtual_pad.product_id,
        power,
        origin: GamepadOrigin::Physical(PhysicalGamepadLink {
            kernel_device: interface.kernel_device.clone(),
            transport: GamepadTransport::SteamDongle,
            identity: ControllerIdentity::Addressed {
                hardware_address: format!(
                    "{}/input{}",
                    interface.dongle_serial, interface.interface_number
                ),
                alias: STEAM_CONTROLLER_NAME.to_owned(),
            },
            player_indicator: PlayerIndicator::Unlit,
            input_route: InputRoute::SteamInput,
        }),
    }
}

impl SteamInputPairing {
    fn paired_roster(&self, roster: &[ConnectedGamepad]) -> Vec<ConnectedGamepad> {
        let mut live_interfaces = self
            .live_interfaces
            .iter()
            .filter(|(_, live)| **live)
            .map(|(interface, _)| interface);
        roster
            .iter()
            .map(|gamepad| {
                if !gamepad.is_steam_virtual_pad() {
                    return gamepad.clone();
                }
                let Some(interface) = live_interfaces.next() else {
                    return gamepad.clone();
                };
                let power = self
                    .battery_by_interface
                    .get(interface)
                    .copied()
                    .unwrap_or(GamepadPower::NoBattery);
                steam_input_controller(gamepad, interface, power)
            })
            .collect()
    }

    fn deliver_roster(&mut self) -> Option<RawGamepadEvent> {
        let paired = self.paired_roster(self.latest_roster.as_deref()?);
        if paired == self.delivered_roster {
            return None;
        }
        self.delivered_roster.clone_from(&paired);
        Some(RawGamepadEvent::Roster(paired))
    }

    pub fn absorb(&mut self, signal: DeviceSignal) -> Option<RawGamepadEvent> {
        match signal {
            DeviceSignal::Raw(RawGamepadEvent::Roster(roster)) => {
                let paired = self.paired_roster(&roster);
                self.latest_roster = Some(roster);
                self.delivered_roster.clone_from(&paired);
                Some(RawGamepadEvent::Roster(paired))
            }
            DeviceSignal::Raw(event) => Some(event),
            DeviceSignal::Dongle(interface, sample) => {
                if let DongleSample::Battery(power) = sample {
                    self.battery_by_interface.insert(interface.clone(), power);
                }
                self.live_interfaces
                    .insert(interface, sample != DongleSample::Silent);
                self.deliver_roster()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::gamepad_controller_input::steam_dongle_hidraw::{
        STEAM_VIRTUAL_PAD_PRODUCT, VALVE_USB_VENDOR,
    };

    fn steam_virtual_pad(slot: usize) -> ConnectedGamepad {
        ConnectedGamepad {
            slot,
            name: format!("Microsoft X-Box 360 pad {slot}"),
            vendor_id: VALVE_USB_VENDOR,
            product_id: STEAM_VIRTUAL_PAD_PRODUCT,
            power: GamepadPower::NoBattery,
            origin: GamepadOrigin::Virtual,
        }
    }

    fn dongle_slot(interface_number: u8) -> DongleInterface {
        DongleInterface {
            interface_number,
            dongle_serial: "FXB99634004CA".to_owned(),
            kernel_device: format!("+hid:0003:28DE:1304.000{interface_number}"),
            hidraw_node: PathBuf::from(format!("/dev/hidraw1{interface_number}")),
        }
    }

    fn roster_of(event: Option<RawGamepadEvent>) -> Vec<ConnectedGamepad> {
        match event {
            Some(RawGamepadEvent::Roster(roster)) => roster,
            other => panic!("expected a roster, got {other:?}"),
        }
    }

    #[test]
    fn a_steam_pad_without_a_live_dongle_slot_stays_virtual() {
        let mut pairing = SteamInputPairing::default();
        pairing.absorb(DeviceSignal::Dongle(dongle_slot(2), DongleSample::Silent));
        let roster = roster_of(pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
            steam_virtual_pad(18),
        ]))));
        assert_eq!(roster, [steam_virtual_pad(18)]);
    }

    #[test]
    fn a_battery_report_turns_the_steam_pad_into_a_steam_controller() {
        let mut pairing = SteamInputPairing::default();
        pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
            steam_virtual_pad(18),
        ])));
        let roster = roster_of(pairing.absorb(DeviceSignal::Dongle(
            dongle_slot(2),
            DongleSample::Battery(GamepadPower::Charging(76)),
        )));
        assert_eq!(roster[0].name, STEAM_CONTROLLER_NAME);
        assert_eq!(roster[0].power, GamepadPower::Charging(76));
        assert!(matches!(
            &roster[0].origin,
            GamepadOrigin::Physical(PhysicalGamepadLink {
                transport: GamepadTransport::SteamDongle,
                input_route: InputRoute::SteamInput,
                ..
            })
        ));
    }

    #[test]
    fn a_streaming_sample_keeps_the_last_battery_level() {
        let mut pairing = SteamInputPairing::default();
        pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
            steam_virtual_pad(18),
        ])));
        pairing.absorb(DeviceSignal::Dongle(
            dongle_slot(2),
            DongleSample::Battery(GamepadPower::Discharging(63)),
        ));
        assert_eq!(
            pairing.absorb(DeviceSignal::Dongle(
                dongle_slot(2),
                DongleSample::Streaming
            )),
            None
        );
    }

    #[test]
    fn steam_pads_pair_with_live_slots_in_order() {
        let mut pairing = SteamInputPairing::default();
        pairing.absorb(DeviceSignal::Dongle(dongle_slot(3), DongleSample::Streaming));
        pairing.absorb(DeviceSignal::Dongle(dongle_slot(2), DongleSample::Streaming));
        let roster = roster_of(pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
            steam_virtual_pad(18),
            steam_virtual_pad(19),
        ]))));
        let addresses: Vec<&str> = roster
            .iter()
            .filter_map(|gamepad| match &gamepad.origin {
                GamepadOrigin::Physical(PhysicalGamepadLink {
                    identity: ControllerIdentity::Addressed {
                        hardware_address, ..
                    },
                    ..
                }) => Some(hardware_address.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(addresses, ["FXB99634004CA/input2", "FXB99634004CA/input3"]);
    }
}
