use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
};

use bytesize::ByteSize;
use tokio::task::spawn_blocking;

const UDEV_DATA: &str = "/run/udev/data";
const CPU_INFO: &str = "/proc/cpuinfo";
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

pub fn reported(raw: &str) -> Option<String> {
    let value = raw.trim();
    let placeholder = FIRMWARE_PLACEHOLDERS
        .iter()
        .any(|placeholder| placeholder.eq_ignore_ascii_case(value));
    (!placeholder).then(|| value.to_owned())
}

pub fn sysfs_attribute(path: impl AsRef<Path>) -> Option<String> {
    reported(&fs::read_to_string(path).ok()?)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UdevProperties(HashMap<String, String>);

impl UdevProperties {
    fn parse(database_entry: &str) -> Self {
        Self(
            database_entry
                .lines()
                .filter_map(|line| line.strip_prefix("E:")?.split_once('='))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        )
    }

    fn read(entry: &str) -> Self {
        fs::read_to_string(Path::new(UDEV_DATA).join(entry))
            .map(|database_entry| Self::parse(&database_entry))
            .unwrap_or_default()
    }

    pub fn dmi() -> Self {
        Self::read("+dmi:id")
    }

    fn of_device(device: &Path) -> Self {
        let Some(sysname) = device.file_name().and_then(|name| name.to_str()) else {
            return Self::default();
        };
        let Some(subsystem) = link_target_name(&device.join("subsystem")) else {
            return Self::default();
        };
        Self::read(&format!("+{subsystem}:{sysname}"))
    }

    fn value(&self, key: &str) -> Option<String> {
        reported(self.0.get(key)?)
    }

    fn vendor(&self) -> Option<String> {
        self.value("ID_VENDOR_FROM_DATABASE")
    }

    fn model(&self) -> Option<String> {
        self.value("ID_MODEL_FROM_DATABASE")
    }
}

fn link_target_name(link: &Path) -> Option<String> {
    Some(fs::read_link(link).ok()?.file_name()?.to_str()?.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusId {
    Pci { vendor: u16, device: u16 },
    Usb { vendor: u16, product: u16 },
}

impl BusId {
    fn of_device(device: &Path) -> Option<Self> {
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

    pub fn bus_name(self) -> &'static str {
        match self {
            BusId::Pci { .. } => "PCI ID",
            BusId::Usb { .. } => "USB ID",
        }
    }
}

impl Display for BusId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            BusId::Pci { vendor, device } => write!(f, "{vendor:04x}:{device:04x}"),
            BusId::Usb { vendor, product } => write!(f, "{vendor:04x}:{product:04x}"),
        }
    }
}

fn hex_attribute(path: &Path) -> Option<u16> {
    let raw = fs::read_to_string(path).ok()?;
    let digits = raw.trim();
    u16::from_str_radix(digits.strip_prefix("0x").unwrap_or(digits), 16).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub bus_id: BusId,
    pub driver: Option<String>,
}

impl Adapter {
    pub fn of_class_device(class_device: impl AsRef<Path>) -> Option<Self> {
        Self::of_device(class_device.as_ref().join("device"))
    }

    pub fn of_device(device: impl AsRef<Path>) -> Option<Self> {
        let function = fs::canonicalize(device).ok()?;
        let (bus_device, bus_id) = function
            .ancestors()
            .find_map(|device| Some((device.to_path_buf(), BusId::of_device(device)?)))?;
        let properties = UdevProperties::of_device(&bus_device);
        Some(Adapter {
            vendor: properties.vendor(),
            model: properties.model(),
            bus_id,
            driver: link_target_name(&function.join("driver")),
        })
    }

    pub fn of_net_interface(interface: &str) -> Option<Self> {
        Self::of_class_device(PathBuf::from("/sys/class/net").join(interface))
    }

    pub fn of_bluetooth_controller(controller: &str) -> Option<Self> {
        Self::of_class_device(PathBuf::from("/sys/class/bluetooth").join(controller))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveIdentity {
    pub model: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
}

impl DriveIdentity {
    pub fn of_controller(controller: &Path) -> Self {
        DriveIdentity {
            model: sysfs_attribute(controller.join("model")),
            serial: sysfs_attribute(controller.join("serial")),
            firmware: sysfs_attribute(controller.join("firmware_rev"))
                .or_else(|| sysfs_attribute(controller.join("rev"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModemIdentity {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub revision: Option<String>,
    pub adapter: Option<Adapter>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PowerSupplyIdentity {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub technology: Option<String>,
}

impl PowerSupplyIdentity {
    pub fn of_supply(supply: &Path) -> Self {
        PowerSupplyIdentity {
            manufacturer: sysfs_attribute(supply.join("manufacturer")),
            model: sysfs_attribute(supply.join("model_name")),
            serial: sysfs_attribute(supply.join("serial_number")),
            technology: sysfs_attribute(supply.join("technology")),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostHardware {
    pub cpu: CpuModel,
    pub memory: MemoryModules,
}

impl HostHardware {
    pub async fn identify() -> Self {
        spawn_blocking(|| HostHardware {
            cpu: CpuModel::current(),
            memory: MemoryModules::current(),
        })
        .await
        .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuSignature {
    X86 {
        family: u32,
        model: u32,
        stepping: u32,
    },
    Arm {
        implementer: u32,
        part: u32,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuModel {
    pub vendor: Option<String>,
    pub name: Option<String>,
    pub signature: Option<CpuSignature>,
}

impl CpuModel {
    pub fn current() -> Self {
        Self::parse(&fs::read_to_string(CPU_INFO).unwrap_or_default())
    }

    fn parse(cpuinfo: &str) -> Self {
        let first_core = cpuinfo.split("\n\n").next().unwrap_or_default();
        let field = |name: &str| {
            first_core.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key.trim() == name).then(|| value.trim())
            })
        };
        CpuModel {
            vendor: field("vendor_id").map(vendor_name),
            name: field("model name").and_then(|name| reported(&without_trademarks(name))),
            signature: Self::x86_signature(&field).or_else(|| Self::arm_signature(&field)),
        }
    }

    fn x86_signature<'a>(field: &impl Fn(&str) -> Option<&'a str>) -> Option<CpuSignature> {
        Some(CpuSignature::X86 {
            family: field("cpu family")?.parse().ok()?,
            model: field("model")?.parse().ok()?,
            stepping: field("stepping")?.parse().ok()?,
        })
    }

    fn arm_signature<'a>(field: &impl Fn(&str) -> Option<&'a str>) -> Option<CpuSignature> {
        let hex = |name| u32::from_str_radix(field(name)?.strip_prefix("0x")?, 16).ok();
        Some(CpuSignature::Arm {
            implementer: hex("CPU implementer")?,
            part: hex("CPU part")?,
        })
    }
}

fn vendor_name(vendor_id: &str) -> String {
    match vendor_id {
        "GenuineIntel" => "Intel".to_owned(),
        "AuthenticAMD" => "AMD".to_owned(),
        other => other.to_owned(),
    }
}

fn without_trademarks(name: &str) -> String {
    ["(R)", "(TM)", "(tm)", "(r)"]
        .iter()
        .fold(name.to_owned(), |name, mark| name.replace(mark, ""))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryModule {
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub kind: Option<String>,
    pub speed_mts: Option<u32>,
    pub size: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryModuleGroup {
    pub module: MemoryModule,
    pub count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryModules(pub Vec<MemoryModuleGroup>);

impl MemoryModules {
    pub fn current() -> Self {
        Self::from_properties(&UdevProperties::dmi())
    }

    fn from_properties(properties: &UdevProperties) -> Self {
        let mut groups: Vec<MemoryModuleGroup> = Vec::new();
        let modules = (0..)
            .map_while(|slot| MemorySlot::read(properties, slot))
            .filter_map(MemorySlot::populated);
        for module in modules {
            match groups.iter_mut().find(|group| group.module == module) {
                Some(group) => group.count += 1,
                None => groups.push(MemoryModuleGroup { module, count: 1 }),
            }
        }
        Self(groups)
    }
}

enum MemorySlot {
    Populated(MemoryModule),
    Empty,
}

impl MemorySlot {
    fn read(properties: &UdevProperties, slot: usize) -> Option<Self> {
        let key = |field: &str| format!("MEMORY_DEVICE_{slot}_{field}");
        properties.0.get(&key("LOCATOR"))?;
        let Some(size) = properties
            .value(&key("SIZE"))
            .and_then(|bytes| bytes.parse().ok())
            .map(ByteSize::b)
        else {
            return Some(MemorySlot::Empty);
        };
        Some(MemorySlot::Populated(MemoryModule {
            manufacturer: properties.value(&key("MANUFACTURER")),
            part_number: properties.value(&key("PART_NUMBER")),
            kind: properties.value(&key("TYPE")),
            speed_mts: properties
                .value(&key("CONFIGURED_SPEED_MTS"))
                .or_else(|| properties.value(&key("SPEED_MTS")))
                .and_then(|speed| speed.parse().ok()),
            size,
        }))
    }

    fn populated(self) -> Option<MemoryModule> {
        match self {
            MemorySlot::Populated(module) => Some(module),
            MemorySlot::Empty => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUNAR_LAKE_CPUINFO: &str = "processor\t: 0\nvendor_id\t: GenuineIntel\ncpu family\t: \
                                      6\nmodel\t\t: 189\nmodel name\t: Intel(R) Core(TM) Ultra 7 \
                                      258V\nstepping\t: 1\n\nprocessor\t: 1\n";

    const SOLDERED_LPDDR5: &str = "\
E:MEMORY_DEVICE_0_SIZE=4294967296
E:MEMORY_DEVICE_0_LOCATOR=Controller0-ChannelA
E:MEMORY_DEVICE_0_TYPE=LPDDR5
E:MEMORY_DEVICE_0_CONFIGURED_SPEED_MTS=8533
E:MEMORY_DEVICE_0_MANUFACTURER=Samsung
E:MEMORY_DEVICE_0_PART_NUMBER=Not Specified
E:MEMORY_DEVICE_1_SIZE=4294967296
E:MEMORY_DEVICE_1_LOCATOR=Controller0-ChannelB
E:MEMORY_DEVICE_1_TYPE=LPDDR5
E:MEMORY_DEVICE_1_CONFIGURED_SPEED_MTS=8533
E:MEMORY_DEVICE_1_MANUFACTURER=Samsung
E:MEMORY_DEVICE_2_LOCATOR=Controller1-ChannelA
E:MEMORY_ARRAY_LOCATION=System Board Or Motherboard
";

    #[test]
    fn x86_cpuinfo_yields_clean_name_and_signature() {
        assert_eq!(
            CpuModel::parse(LUNAR_LAKE_CPUINFO),
            CpuModel {
                vendor: Some("Intel".to_owned()),
                name: Some("Intel Core Ultra 7 258V".to_owned()),
                signature: Some(CpuSignature::X86 {
                    family: 6,
                    model: 189,
                    stepping: 1
                }),
            }
        );
    }

    #[test]
    fn arm_cpuinfo_yields_implementer_and_part() {
        let cpuinfo = "processor\t: 0\nCPU implementer\t: 0x41\nCPU part\t: 0xd0b\n";
        assert_eq!(
            CpuModel::parse(cpuinfo).signature,
            Some(CpuSignature::Arm {
                implementer: 0x41,
                part: 0xd0b
            })
        );
    }

    #[test]
    fn identical_modules_are_counted_and_empty_slots_skipped() {
        let modules = MemoryModules::from_properties(&UdevProperties::parse(SOLDERED_LPDDR5));
        assert_eq!(
            modules,
            MemoryModules(vec![MemoryModuleGroup {
                module: MemoryModule {
                    manufacturer: Some("Samsung".to_owned()),
                    part_number: None,
                    kind: Some("LPDDR5".to_owned()),
                    speed_mts: Some(8533),
                    size: ByteSize::gib(4),
                },
                count: 2,
            }])
        );
    }

    #[test]
    fn firmware_placeholders_are_not_reported() {
        assert_eq!(reported("  To Be Filled By O.E.M. "), None);
        assert_eq!(reported("not specified"), None);
        assert_eq!(reported(" BYD "), Some("BYD".to_owned()));
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
