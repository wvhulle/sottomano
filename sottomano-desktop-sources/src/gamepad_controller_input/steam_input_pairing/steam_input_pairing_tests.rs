use std::path::PathBuf;

use super::*;
use crate::{
    gamepad_controller_input::{
        evdev_gamepad_devices::ButtonEdge,
        steam_dongle_hidraw::{STEAM_VIRTUAL_PAD_PRODUCT, VALVE_USB_VENDOR},
        steam_input_facts::{FirmwareStatus, PairingEvidence, RadioReading},
    },
    steam_client_files::{
        steam_log_lines::{LogLineShape, controller_log_lines::ControllerLogLine},
        virtual_gamepad_slots::{SteamVirtualSlot, VirtualSlotControllerType},
    },
};
use sottomano_toml_config::indicator_groups::gamepad_bindings::GamepadButton;

fn steam_virtual_pad(evdev_slot: usize, xinput_slot: u8) -> ConnectedGamepad {
    ConnectedGamepad {
        slot: evdev_slot,
        name: format!("Microsoft X-Box 360 pad {xinput_slot}"),
        vendor_id: VALVE_USB_VENDOR,
        product_id: STEAM_VIRTUAL_PAD_PRODUCT,
        power: GamepadPower::NoBattery,
        origin: GamepadOrigin::Virtual,
    }
}

fn dongle_slot(interface_number: u8) -> DongleInterface {
    DongleInterface {
        interface_number,
        dongle_serial: "FXB0000000009".to_owned(),
        kernel_device: format!("+hid:0003:28DE:1304.000{interface_number}"),
        hidraw_node: PathBuf::from(format!("/dev/hidraw1{interface_number}")),
    }
}

fn virtual_slot(xinput_slot: u8, controller_type: VirtualSlotControllerType) -> SteamVirtualSlot {
    SteamVirtualSlot {
        slot: xinput_slot,
        physical_name: "Steam Controller".to_owned(),
        physical_vendor: VALVE_USB_VENDOR,
        physical_product: 0x1302,
        controller_type,
        steam_handle: 0xfa00_0000_0001 + u64::from(xinput_slot),
    }
}

fn facts_with(slots: Vec<SteamVirtualSlot>, controller_log_lines: &[&str]) -> SteamControllerFacts {
    let mut facts = SteamControllerFacts {
        virtual_slots: slots,
        ..SteamControllerFacts::default()
    };
    for line in controller_log_lines {
        facts
            .controller_log
            .absorb(ControllerLogLine::from(LogLineShape::from(*line)));
    }
    facts
}

fn latest_roster(events: Vec<RawGamepadEvent>) -> Vec<ConnectedGamepad> {
    events
        .into_iter()
        .rev()
        .find_map(|event| match event {
            RawGamepadEvent::Roster(roster) => Some(roster),
            _ => None,
        })
        .expect("a roster update")
}

fn hardware_address(gamepad: &ConnectedGamepad) -> Option<&str> {
    match &gamepad.origin {
        GamepadOrigin::Physical(PhysicalGamepadLink {
            identity: ControllerIdentity::Addressed {
                hardware_address, ..
            },
            ..
        }) => Some(hardware_address),
        GamepadOrigin::Physical(_) | GamepadOrigin::Virtual => None,
    }
}

fn kernel_device(gamepad: &ConnectedGamepad) -> Option<&str> {
    match &gamepad.origin {
        GamepadOrigin::Physical(link) => Some(&link.kernel_device),
        GamepadOrigin::Virtual => None,
    }
}

const SECOND_SLOT_LOG: [&str; 6] = [
    "[2026-11-19 21:34:11] Local Device Found",
    "  type: 28de 1304",
    "  path: /dev/hidraw13",
    "[2026-11-19 21:34:11] !! Steam controller device opened for index 3.",
    "[2026-11-20 11:00:17] SetLocalControllerConnectionState for 3 to wireless_established because Event from device",
    "[2026-11-20 11:00:17] Steam Controller reserving XInput slot 1",
];

#[test]
fn another_controllers_steam_pad_is_never_a_steam_controller() {
    let mut pairing = SteamInputPairing::default();
    pairing.absorb(DeviceSignal::SteamFacts(Box::new(facts_with(
        vec![virtual_slot(0, VirtualSlotControllerType::OtherController)],
        &[],
    ))));
    pairing.absorb(DeviceSignal::Dongle(dongle_slot(2), DongleSample::Streaming));
    let roster = latest_roster(pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
        steam_virtual_pad(18, 0),
    ]))));
    assert_eq!(roster, [steam_virtual_pad(18, 0)]);
}

#[test]
fn the_controller_log_pairs_a_pad_with_its_dongle_slot() {
    let mut pairing = SteamInputPairing::default();
    pairing.absorb(DeviceSignal::Dongle(dongle_slot(2), DongleSample::Streaming));
    pairing.absorb(DeviceSignal::Dongle(
        dongle_slot(3),
        DongleSample::Battery(GamepadPower::Charging(76)),
    ));
    pairing.absorb(DeviceSignal::SteamFacts(Box::new(facts_with(
        vec![
            virtual_slot(0, VirtualSlotControllerType::SteamController),
            virtual_slot(1, VirtualSlotControllerType::SteamController),
        ],
        &SECOND_SLOT_LOG,
    ))));
    let roster = latest_roster(pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
        steam_virtual_pad(18, 0),
        steam_virtual_pad(19, 1),
    ]))));
    assert_eq!(roster[1].name, STEAM_CONTROLLER_NAME);
    assert_eq!(roster[1].power, GamepadPower::Charging(76));
    assert_eq!(hardware_address(&roster[1]), Some("FXA0000000002"));
    assert_eq!(hardware_address(&roster[0]), Some("FXA0000000001"));
    assert_eq!(kernel_device(&roster[0]), Some("+hid:0003:28DE:1304.0002"));
}

#[test]
fn a_lone_pad_and_a_lone_live_slot_pair_without_the_log() {
    let mut pairing = SteamInputPairing::default();
    pairing.absorb(DeviceSignal::SteamFacts(Box::new(facts_with(
        vec![virtual_slot(0, VirtualSlotControllerType::SteamController)],
        &[],
    ))));
    pairing.absorb(DeviceSignal::Dongle(dongle_slot(2), DongleSample::Streaming));
    let roster = latest_roster(pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
        steam_virtual_pad(18, 0),
    ]))));
    assert!(matches!(
        &roster[0].origin,
        GamepadOrigin::Physical(PhysicalGamepadLink {
            input_route: InputRoute::SteamInput(SteamInputFacts {
                pairing: PairingEvidence::SoleRemainingCandidate,
                radio: RadioReading::Unattributed,
                controller_firmware: FirmwareStatus::Current,
                dongle_firmware: FirmwareStatus::Current,
            }),
            ..
        })
    ));
}

#[test]
fn a_button_edge_pairs_the_pad_with_the_slot_that_changed_with_it() {
    let mut pairing = SteamInputPairing::default();
    pairing.absorb(DeviceSignal::SteamFacts(Box::new(facts_with(
        vec![
            virtual_slot(0, VirtualSlotControllerType::SteamController),
            virtual_slot(1, VirtualSlotControllerType::SteamController),
        ],
        &[],
    ))));
    pairing.absorb(DeviceSignal::Dongle(dongle_slot(2), DongleSample::Streaming));
    pairing.absorb(DeviceSignal::Dongle(dongle_slot(3), DongleSample::Streaming));
    let unpaired = latest_roster(pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Roster(vec![
        steam_virtual_pad(18, 0),
        steam_virtual_pad(19, 1),
    ]))));
    assert_eq!(unpaired[1], steam_virtual_pad(19, 1));
    pairing.absorb(DeviceSignal::DongleButtonsChanged(
        dongle_slot(3),
        Instant::now(),
    ));
    let events = pairing.absorb(DeviceSignal::Raw(RawGamepadEvent::Button {
        slot: 19,
        button: GamepadButton::South,
        edge: ButtonEdge::Pressed,
    }));
    let paired = latest_roster(events);
    assert_eq!(kernel_device(&paired[1]), Some("+hid:0003:28DE:1304.0003"));
    assert_eq!(kernel_device(&paired[0]), Some("+hid:0003:28DE:1304.0002"));
}
