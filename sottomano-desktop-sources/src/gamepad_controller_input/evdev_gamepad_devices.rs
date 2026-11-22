use std::{collections::HashMap, path::Path};

use evdev::{BusType, Device, InputId};
use sottomano_toml_config::indicator_groups::gamepad_bindings::{GamepadButton, GamepadStick};

use super::{
    accelerometer_report_timing::ReportTiming,
    player_indicator_leds::PlayerIndicator,
    steam_dongle_hidraw::{STEAM_PUCK_PRODUCTS, STEAM_VIRTUAL_PAD_PRODUCT, VALVE_USB_VENDOR},
    steam_input_facts::SteamInputFacts,
};
use crate::{
    battery_power_profiles::upower_battery_state::ChargeStatus,
    delivery_cadence_drivers::ChangeWatchFailure,
    network_connectivity::hid_sysfs_bluetooth_peripherals::DeviceBattery,
    system_protocol_clients::sysfs_attribute_reads,
};

mod gamepad_event_translation;
pub mod gamepad_node_discovery;

const HID_DEVICE_DIRECTORY: &str = "/sys/bus/hid/devices";
const USB_HID_DEVICE_PREFIX: &str = "0003:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadPower {
    NoBattery,
    Discharging(u8),
    Charging(u8),
    Full,
}

impl From<DeviceBattery> for GamepadPower {
    fn from(battery: DeviceBattery) -> Self {
        match battery {
            DeviceBattery::Unreported => GamepadPower::NoBattery,
            DeviceBattery::Reported {
                level_percent,
                charging: ChargeStatus::Charging,
            } => GamepadPower::Charging(level_percent),
            DeviceBattery::Reported {
                charging: ChargeStatus::Full,
                ..
            } => GamepadPower::Full,
            DeviceBattery::Reported { level_percent, .. } => {
                GamepadPower::Discharging(level_percent)
            }
        }
    }
}

impl From<usize> for GamepadPower {
    fn from(slot: usize) -> Self {
        sysfs_attribute_reads::list_dir(format!(
            "/sys/class/input/event{slot}/device/device/power_supply"
        ))
        .first()
        .map_or(GamepadPower::NoBattery, |supply| {
            DeviceBattery::read_power_supply(supply).into()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbCableState {
    Absent,
    IgnoredByKernel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTransport {
    Bluetooth { usb_cable: UsbCableState },
    Usb,
    SteamDongle,
    Other,
}

impl GamepadTransport {
    fn refreshed(self, identity: &ControllerIdentity) -> Self {
        match (self, identity) {
            (
                GamepadTransport::Bluetooth { .. },
                ControllerIdentity::Addressed {
                    hardware_address, ..
                },
            ) => GamepadTransport::Bluetooth {
                usb_cable: usb_cable_state(hardware_address),
            },
            (transport, _) => transport,
        }
    }
}

impl From<InputId> for GamepadTransport {
    fn from(input_id: InputId) -> Self {
        match input_id.bus_type() {
            BusType::BUS_BLUETOOTH => GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::Absent,
            },
            BusType::BUS_USB
                if input_id.vendor() == VALVE_USB_VENDOR
                    && STEAM_PUCK_PRODUCTS.contains(&input_id.product()) =>
            {
                GamepadTransport::SteamDongle
            }
            BusType::BUS_USB => GamepadTransport::Usb,
            _ => GamepadTransport::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputRoute {
    Kernel,
    SteamInput(SteamInputFacts),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerIdentity {
    Addressed {
        hardware_address: String,
        alias: String,
    },
    Unaddressed,
}

impl ControllerIdentity {
    fn refreshed(&self, bluetooth_aliases: &HashMap<String, String>) -> Self {
        match self {
            ControllerIdentity::Addressed {
                hardware_address,
                alias,
            } => ControllerIdentity::Addressed {
                hardware_address: hardware_address.clone(),
                alias: bluetooth_aliases
                    .get(&hardware_address.to_lowercase())
                    .unwrap_or(alias)
                    .clone(),
            },
            ControllerIdentity::Unaddressed => ControllerIdentity::Unaddressed,
        }
    }
}

impl From<&Device> for ControllerIdentity {
    fn from(device: &Device) -> Self {
        match device.unique_name().filter(|unique| !unique.is_empty()) {
            Some(hardware_address) => ControllerIdentity::Addressed {
                hardware_address: hardware_address.to_owned(),
                alias: device.name().unwrap_or(hardware_address).to_owned(),
            },
            None => ControllerIdentity::Unaddressed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalGamepadLink {
    pub kernel_device: String,
    pub transport: GamepadTransport,
    pub identity: ControllerIdentity,
    pub player_indicator: PlayerIndicator,
    pub input_route: InputRoute,
}

impl PhysicalGamepadLink {
    fn refreshed(&self, slot: usize, bluetooth_aliases: &HashMap<String, String>) -> Self {
        let identity = self.identity.refreshed(bluetooth_aliases);
        PhysicalGamepadLink {
            kernel_device: self.kernel_device.clone(),
            transport: self.transport.refreshed(&identity),
            identity,
            player_indicator: PlayerIndicator::from(slot),
            input_route: self.input_route,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamepadOrigin {
    Physical(PhysicalGamepadLink),
    Virtual,
}

impl GamepadOrigin {
    fn probe_input_node(slot: usize, device: &Device) -> Self {
        kernel_device_of_input_node(slot).map_or(GamepadOrigin::Virtual, |kernel_device| {
            GamepadOrigin::Physical(PhysicalGamepadLink {
                kernel_device,
                transport: GamepadTransport::from(device.input_id()),
                identity: ControllerIdentity::from(device),
                player_indicator: PlayerIndicator::from(slot),
                input_route: InputRoute::Kernel,
            })
        })
    }

    fn refreshed(&self, slot: usize, bluetooth_aliases: &HashMap<String, String>) -> Self {
        match self {
            GamepadOrigin::Physical(link) => {
                GamepadOrigin::Physical(link.refreshed(slot, bluetooth_aliases))
            }
            GamepadOrigin::Virtual => GamepadOrigin::Virtual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedGamepad {
    pub slot: usize,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub power: GamepadPower,
    pub origin: GamepadOrigin,
}

impl ConnectedGamepad {
    #[must_use]
    pub fn is_steam_virtual_pad(&self) -> bool {
        is_steam_virtual_pad(self.vendor_id, self.product_id, &self.origin)
    }
}

fn is_steam_virtual_pad(vendor_id: u16, product_id: u16, origin: &GamepadOrigin) -> bool {
    *origin == GamepadOrigin::Virtual
        && vendor_id == VALVE_USB_VENDOR
        && product_id == STEAM_VIRTUAL_PAD_PRODUCT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEdge {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, PartialEq)]
pub enum RawGamepadEvent {
    DiscoveryFailed(ChangeWatchFailure),
    Roster(Vec<ConnectedGamepad>),
    Button {
        slot: usize,
        button: GamepadButton,
        edge: ButtonEdge,
    },
    Stick {
        slot: usize,
        stick: GamepadStick,
        axis: PlaneAxis,
        deflection: f32,
    },
    ReportTiming {
        slot: usize,
        timing: ReportTiming,
    },
}

fn kernel_device_of_input_node(slot: usize) -> Option<String> {
    let parent = Path::new(&format!("/sys/class/input/event{slot}/device/device"))
        .canonicalize()
        .inspect_err(|resolve_failure| {
            tracing::debug!(
                "gamepad: parent device of event{slot} unresolvable: {resolve_failure}"
            );
        })
        .ok()?;
    let subsystem = parent
        .join("subsystem")
        .canonicalize()
        .inspect_err(|resolve_failure| {
            tracing::debug!(
                "gamepad: subsystem of {} unresolvable: {resolve_failure}",
                parent.display()
            );
        })
        .ok()?;
    Some(format!(
        "+{}:{}",
        subsystem.file_name()?.to_str()?,
        parent.file_name()?.to_str()?
    ))
}

fn usb_cable_state(hardware_address: &str) -> UsbCableState {
    if sysfs_attribute_reads::list_dir(HID_DEVICE_DIRECTORY)
        .into_iter()
        .filter(|hid_device| {
            hid_device
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(USB_HID_DEVICE_PREFIX))
        })
        .any(|hid_device| {
            !hid_device.join("driver").exists()
                && sysfs_attribute_reads::uevent_field(&hid_device.join("uevent"), "HID_UNIQ")
                    .is_some_and(|unique| unique.eq_ignore_ascii_case(hardware_address))
        })
    {
        UsbCableState::IgnoredByKernel
    } else {
        UsbCableState::Absent
    }
}
