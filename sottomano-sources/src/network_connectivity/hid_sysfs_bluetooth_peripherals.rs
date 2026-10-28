use std::path::Path;

use crate::system_protocol_clients::sysfs_attribute_reads::{
    list_dir, read_i64, read_trimmed, uevent_field,
};

const HID_DEVICE_DIRECTORY: &str = "/sys/bus/hid/devices";
const BLUETOOTH_HID_BUS_PREFIX: &str = "0005:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargingState {
    Unknown,
    Charging,
    Discharging,
    NotCharging,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceBattery {
    Unreported,
    Reported {
        level_percent: u8,
        charging: ChargingState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidPeripheral {
    pub kernel_device: String,
    pub battery: DeviceBattery,
}

impl ChargingState {
    fn of_supply_status(status: &str) -> Self {
        match status {
            "Charging" => ChargingState::Charging,
            "Discharging" => ChargingState::Discharging,
            "Not charging" => ChargingState::NotCharging,
            "Full" => ChargingState::Full,
            _ => ChargingState::Unknown,
        }
    }
}

impl DeviceBattery {
    #[must_use]
    pub fn or_else(self, fallback: impl FnOnce() -> DeviceBattery) -> DeviceBattery {
        match self {
            DeviceBattery::Reported { .. } => self,
            DeviceBattery::Unreported => fallback(),
        }
    }

    fn of_power_supply(supply: &Path) -> Self {
        let Some(capacity) = read_i64(supply.join("capacity")) else {
            return DeviceBattery::Unreported;
        };
        DeviceBattery::Reported {
            level_percent: u8::try_from(capacity.clamp(0, 100)).unwrap_or_default(),
            charging: read_trimmed(supply.join("status"))
                .as_deref()
                .map_or(ChargingState::Unknown, ChargingState::of_supply_status),
        }
    }

    fn of_hid_device(hid_device: &Path) -> Self {
        list_dir(hid_device.join("power_supply"))
            .iter()
            .map(|supply| DeviceBattery::of_power_supply(supply))
            .find(|battery| matches!(battery, DeviceBattery::Reported { .. }))
            .unwrap_or(DeviceBattery::Unreported)
    }
}

fn is_bluetooth_hid_device(hid_device: &Path) -> bool {
    hid_device
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(BLUETOOTH_HID_BUS_PREFIX))
}

fn hid_peripheral_of(hid_device: &Path) -> Option<HidPeripheral> {
    let sysfs_name = hid_device.file_name()?.to_str()?;
    Some(HidPeripheral {
        kernel_device: format!("+hid:{sysfs_name}"),
        battery: DeviceBattery::of_hid_device(hid_device),
    })
}

#[must_use]
pub fn hid_peripherals_with_address(hardware_address: &str) -> Vec<HidPeripheral> {
    list_dir(HID_DEVICE_DIRECTORY)
        .into_iter()
        .filter(|hid_device| is_bluetooth_hid_device(hid_device))
        .filter(|hid_device| {
            uevent_field(&hid_device.join("uevent"), "HID_UNIQ")
                .is_some_and(|unique| unique.eq_ignore_ascii_case(hardware_address))
        })
        .filter_map(|hid_device| hid_peripheral_of(&hid_device))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_status_strings_map_to_charging_states() {
        assert_eq!(
            ChargingState::of_supply_status("Not charging"),
            ChargingState::NotCharging
        );
        assert_eq!(
            ChargingState::of_supply_status("Charging"),
            ChargingState::Charging
        );
        assert_eq!(
            ChargingState::of_supply_status("Unknown"),
            ChargingState::Unknown
        );
    }

    #[test]
    fn a_reported_battery_wins_over_the_fallback() {
        let reported = DeviceBattery::Reported {
            level_percent: 40,
            charging: ChargingState::Discharging,
        };
        assert_eq!(reported.or_else(|| DeviceBattery::Unreported), reported);
        assert_eq!(DeviceBattery::Unreported.or_else(|| reported), reported);
    }

    #[test]
    fn only_bluetooth_bus_hid_devices_are_considered() {
        assert!(is_bluetooth_hid_device(Path::new(
            "/sys/bus/hid/devices/0005:054C:0CE6.0007"
        )));
        assert!(!is_bluetooth_hid_device(Path::new(
            "/sys/bus/hid/devices/0003:054C:0CE6.0002"
        )));
    }
}
