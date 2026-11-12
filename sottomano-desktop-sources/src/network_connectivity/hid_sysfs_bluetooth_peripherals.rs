use std::path::Path;

use crate::system_protocol_clients::sysfs_attribute_reads::{
    list_dir, read_i64, read_trimmed, uevent_field,
};

const HID_DEVICE_DIRECTORY: &str = "/sys/bus/hid/devices";

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

impl From<&str> for ChargingState {
    fn from(status: &str) -> Self {
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
            level_percent: u8::try_from(capacity.clamp(0, 100))
                .expect("a percentage clamped to 0..=100 fits a byte"),
            charging: read_trimmed(supply.join("status"))
                .as_deref()
                .map_or(ChargingState::Unknown, ChargingState::from),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HidBus {
    Usb,
    Bluetooth,
    Other,
}

impl From<&Path> for HidBus {
    fn from(hid_device: &Path) -> Self {
        let bus_code = hid_device
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.split_once(':'))
            .map(|(bus_code, _)| bus_code);
        match bus_code {
            Some("0003") => HidBus::Usb,
            Some("0005") => HidBus::Bluetooth,
            _ => HidBus::Other,
        }
    }
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
        .filter(|hid_device| HidBus::from(hid_device.as_path()) == HidBus::Bluetooth)
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
            ChargingState::from("Not charging"),
            ChargingState::NotCharging
        );
        assert_eq!(ChargingState::from("Charging"), ChargingState::Charging);
        assert_eq!(ChargingState::from("Unknown"), ChargingState::Unknown);
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
    fn hid_bus_follows_the_sysfs_bus_code() {
        assert_eq!(
            HidBus::from(Path::new("/sys/bus/hid/devices/0005:054C:0CE6.0007")),
            HidBus::Bluetooth
        );
        assert_eq!(
            HidBus::from(Path::new("/sys/bus/hid/devices/0003:054C:0CE6.0002")),
            HidBus::Usb
        );
    }
}
