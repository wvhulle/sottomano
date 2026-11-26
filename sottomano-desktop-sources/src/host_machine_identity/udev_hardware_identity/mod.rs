mod cpu_cache_levels;
mod cpu_model_identity;
mod host_hardware_snapshot;
mod memory_slot_modules;

use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
};

pub use cpu_cache_levels::{
    CacheInstances, CacheKind, CacheLevel, CachePosition, UnknownCacheKind,
};
pub use cpu_model_identity::{CpuArchitecture, CpuCoreCounts, CpuIdentity, CpuModel, CpuVendor};
pub use host_hardware_snapshot::{HostHardware, HostHardwareError};
pub use memory_slot_modules::{MemoryModule, MemorySlotModule, MemoryTransferRate};

const UDEV_DATA: &str = "/run/udev/data";
const FIRMWARE_PLACEHOLDERS: [&str; 9] = [
    "",
    "Not Specified",
    "Unknown",
    "None",
    "N/A",
    "Default string",
    "To Be Filled By O.E.M.",
    "System Product Name",
    "0000",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum HardwareText {
    Reported(String),
    #[default]
    Unreported,
}

fn firmware_placeholder(trimmed: &str) -> bool {
    FIRMWARE_PLACEHOLDERS
        .iter()
        .any(|placeholder| placeholder.eq_ignore_ascii_case(trimmed))
}

#[must_use]
pub fn reported(raw: &str) -> HardwareText {
    let trimmed = raw.trim();
    if firmware_placeholder(trimmed) {
        HardwareText::Unreported
    } else {
        HardwareText::Reported(trimmed.to_owned())
    }
}

pub fn sysfs_attribute(path: impl AsRef<Path>) -> HardwareText {
    let path = path.as_ref();
    fs::read_to_string(path).map_or_else(
        |read_failure| {
            tracing::debug!(
                "hardware identity: cannot read {}: {read_failure}",
                path.display()
            );
            HardwareText::Unreported
        },
        |raw| reported(&raw),
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UdevProperties {
    properties_by_key: HashMap<String, String>,
}

impl UdevProperties {
    fn parse(database_entry: &str) -> Self {
        Self {
            properties_by_key: database_entry
                .lines()
                .filter_map(|line| line.strip_prefix("E:")?.split_once('='))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        }
    }

    fn read(entry: &str) -> Self {
        fs::read_to_string(Path::new(UDEV_DATA).join(entry)).map_or_else(
            |read_failure| {
                tracing::debug!(
                    "udev properties: cannot read {UDEV_DATA} entry {entry}: {read_failure}"
                );
                Self::default()
            },
            |database_entry| Self::parse(&database_entry),
        )
    }

    #[must_use]
    pub fn dmi() -> Self {
        Self::read("+dmi:id")
    }

    #[must_use]
    pub fn block_device(major_minor: &str) -> Self {
        Self::read(&format!("b{major_minor}"))
    }

    #[must_use]
    pub fn value(&self, key: &str) -> Option<String> {
        let trimmed = self.properties_by_key.get(key)?.trim();
        (!firmware_placeholder(trimmed)).then(|| trimmed.to_owned())
    }

    #[must_use]
    pub fn text(&self, key: &str) -> HardwareText {
        self.properties_by_key
            .get(key)
            .map_or(HardwareText::Unreported, |raw| reported(raw))
    }
}

impl From<&Path> for UdevProperties {
    fn from(device: &Path) -> Self {
        let Some(sysname) = device.file_name().and_then(|name| name.to_str()) else {
            return Self::default();
        };
        let Some(subsystem) = link_target_name(&device.join("subsystem")) else {
            return Self::default();
        };
        Self::read(&format!("+{subsystem}:{sysname}"))
    }
}

fn link_target_name(link: &Path) -> Option<String> {
    let target = fs::read_link(link)
        .inspect_err(|link_failure| {
            tracing::debug!(
                "hardware identity: cannot read link {}: {link_failure}",
                link.display()
            );
        })
        .ok()?;
    Some(target.file_name()?.to_str()?.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusId {
    Pci { vendor: u16, device: u16 },
    Usb { vendor: u16, product: u16 },
}

impl BusId {
    fn read_sysfs_device(device: &Path) -> Option<Self> {
        match link_target_name(&device.join("subsystem"))?.as_str() {
            "pci" => Some(BusId::Pci {
                vendor: hex_attribute(&device.join("vendor"))?,
                device: hex_attribute(&device.join("device"))?,
            }),
            "usb" => Some(BusId::Usb {
                vendor: hex_attribute(&device.join("idVendor"))?,
                product: hex_attribute(&device.join("idProduct"))?,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn lookup_url(self) -> String {
        match self {
            BusId::Pci { vendor, device } => {
                format!("https://pci-ids.ucw.cz/read/PC/{vendor:04x}/{device:04x}")
            }
            BusId::Usb { vendor, product } => {
                format!("https://usb-ids.gowdy.us/read/UD/{vendor:04x}/{product:04x}")
            }
        }
    }

    #[must_use]
    pub fn bus_name(self) -> &'static str {
        match self {
            BusId::Pci { .. } => "PCI ID",
            BusId::Usb { .. } => "USB ID",
        }
    }
}

impl Display for BusId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            BusId::Pci { vendor, device } => write!(formatter, "{vendor:04x}:{device:04x}"),
            BusId::Usb { vendor, product } => write!(formatter, "{vendor:04x}:{product:04x}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentPciVendor {
    Identified { vendor_code: u16 },
    Unidentified,
}

pub(crate) fn pci_parent_vendor(device: &Path) -> ParentPciVendor {
    let parent = device.join("device");
    if link_target_name(&parent.join("subsystem")).as_deref() != Some("pci") {
        return ParentPciVendor::Unidentified;
    }
    hex_attribute(&parent.join("vendor")).map_or(ParentPciVendor::Unidentified, |vendor_code| {
        ParentPciVendor::Identified { vendor_code }
    })
}

fn hex_attribute(path: &Path) -> Option<u16> {
    let raw = fs::read_to_string(path)
        .inspect_err(|read_failure| {
            tracing::debug!(
                "hardware identity: cannot read {}: {read_failure}",
                path.display()
            );
        })
        .ok()?;
    let digits = raw.trim();
    u16::from_str_radix(digits.strip_prefix("0x").unwrap_or(digits), 16)
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "hardware identity: invalid hexadecimal id {digits:?} in {}: {parse_failure}",
                path.display()
            );
        })
        .ok()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub vendor: HardwareText,
    pub model: HardwareText,
    pub serial: HardwareText,
    pub revision: HardwareText,
}

impl DeviceIdentity {
    #[must_use]
    pub fn read_drive_controller(controller: &Path) -> Self {
        DeviceIdentity {
            vendor: HardwareText::Unreported,
            model: sysfs_attribute(controller.join("model")),
            serial: sysfs_attribute(controller.join("serial")),
            revision: match sysfs_attribute(controller.join("firmware_rev")) {
                HardwareText::Unreported => sysfs_attribute(controller.join("rev")),
                firmware_revision @ HardwareText::Reported(_) => firmware_revision,
            },
        }
    }

    #[must_use]
    pub fn read_power_supply(supply: &Path) -> Self {
        DeviceIdentity {
            vendor: sysfs_attribute(supply.join("manufacturer")),
            model: sysfs_attribute(supply.join("model_name")),
            serial: sysfs_attribute(supply.join("serial_number")),
            revision: HardwareText::Unreported,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    pub identity: DeviceIdentity,
    pub bus_id: BusId,
    pub driver: KernelDriver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelDriver {
    Bound(String),
    Unbound,
}

impl Adapter {
    pub fn read_class_device(class_device: impl AsRef<Path>) -> Option<Self> {
        Self::read_sysfs_device(class_device.as_ref().join("device"))
    }

    pub fn read_sysfs_device(device: impl AsRef<Path>) -> Option<Self> {
        let device = device.as_ref();
        let function = fs::canonicalize(device)
            .inspect_err(|resolve_failure| {
                tracing::debug!(
                    "hardware identity: cannot resolve device {}: {resolve_failure}",
                    device.display()
                );
            })
            .ok()?;
        let (bus_device, bus_id) = function
            .ancestors()
            .find_map(|device| Some((device.to_path_buf(), BusId::read_sysfs_device(device)?)))?;
        let properties = UdevProperties::from(bus_device.as_path());
        Some(Adapter {
            identity: DeviceIdentity {
                vendor: properties.text("ID_VENDOR_FROM_DATABASE"),
                model: properties.text("ID_MODEL_FROM_DATABASE"),
                serial: HardwareText::Unreported,
                revision: HardwareText::Unreported,
            },
            bus_id,
            driver: link_target_name(&function.join("driver"))
                .map_or(KernelDriver::Unbound, KernelDriver::Bound),
        })
    }

    #[must_use]
    pub fn read_net_interface(interface: &str) -> Option<Self> {
        Self::read_class_device(PathBuf::from("/sys/class/net").join(interface))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_placeholders_are_not_reported() {
        assert_eq!(
            reported("  To Be Filled By O.E.M. "),
            HardwareText::Unreported
        );
        assert_eq!(reported("not specified"), HardwareText::Unreported);
        assert_eq!(reported(" BYD "), HardwareText::Reported("BYD".to_owned()));
    }

    #[test]
    fn bus_ids_link_to_the_id_databases() {
        let wifi = BusId::Pci {
            vendor: 0x8086,
            device: 0x272b,
        };
        assert_eq!(wifi.to_string(), "8086:272b");
        assert_eq!(
            wifi.lookup_url(),
            "https://pci-ids.ucw.cz/read/PC/8086/272b"
        );
    }
}
