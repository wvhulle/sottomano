use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use bytesize::ByteSize;
use iced::futures::Stream;
use serde_json::Value;

use crate::{
    data::source::{
        DeliveryCadence, Panicked, ReadingSource, SourceDelivery, blocking_poll, bus::DesktopBuses,
        command, hostname::MachineInfo, polled_delivery_stream,
    },
    widgets::tooltip_body::Section,
};

const UDEV_DATA: &str = "/run/udev/data";
const CPU_INFO: &str = "/proc/cpuinfo";
const CPU_DEVICES: &str = "/sys/devices/system/cpu";
const INXI_PROGRAM: &str = "inxi";
const INXI_CPU_REPORT_ARGS: [&str; 8] = [
    "--tty",
    "--cpu",
    "-xxx",
    "-a",
    "--output",
    "json",
    "--output-file",
    "print",
];
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
    pub machine: MachineInfo,
}

#[derive(Debug, Clone)]
pub enum HostHardwareError {
    HostnamedUnreadable(zbus::Error),
    Panicked,
}

impl From<zbus::Error> for HostHardwareError {
    fn from(error: zbus::Error) -> Self {
        Self::HostnamedUnreadable(error)
    }
}

impl From<Panicked> for HostHardwareError {
    fn from(Panicked: Panicked) -> Self {
        Self::Panicked
    }
}

impl Display for HostHardwareError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::HostnamedUnreadable(error) => write!(f, "cannot read hostnamed: {error}"),
            Self::Panicked => write!(f, "hardware read panicked"),
        }
    }
}

impl From<HostHardwareError> for Section {
    fn from(error: HostHardwareError) -> Self {
        Section::error(error)
    }
}

impl ReadingSource for HostHardware {
    type Error = HostHardwareError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "host hardware";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_hours(1));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl HostHardware {
    async fn poll(buses: DesktopBuses) -> Result<Self, HostHardwareError> {
        let machine = MachineInfo::read(&buses.system).await?;
        blocking_poll(move || {
            Ok(HostHardware {
                cpu: CpuModel::current(),
                memory: MemoryModules::current(),
                machine,
            })
        })
        .await
    }
}

fn cpuinfo_field<'a>(core: &'a str, name: &str) -> Option<&'a str> {
    core.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then(|| value.trim())
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuModel {
    pub identity: CpuIdentity,
    pub architecture: CpuArchitecture,
    pub topology: CpuCoreCounts,
    pub caches: Vec<CacheLevel>,
}

impl CpuModel {
    pub fn current() -> Self {
        let cpuinfo = fs::read_to_string(CPU_INFO).unwrap_or_default();
        let first_core = cpuinfo.split("\n\n").next().unwrap_or_default();
        CpuModel {
            identity: CpuIdentity::parse(first_core),
            architecture: CpuArchitecture::current(),
            topology: CpuCoreCounts::parse(first_core),
            caches: cache_levels(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuIdentity {
    pub name: Option<String>,
    pub microcode: Option<String>,
}

impl CpuIdentity {
    fn parse(first_core: &str) -> Self {
        let field = |name| cpuinfo_field(first_core, name).map(str::to_owned);
        CpuIdentity {
            name: field("model name"),
            microcode: field("microcode"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuCoreCounts {
    pub cores: Option<u32>,
    pub threads: Option<u32>,
}

impl CpuCoreCounts {
    fn parse(first_core: &str) -> Self {
        let count = |name| cpuinfo_field(first_core, name)?.parse().ok();
        CpuCoreCounts {
            cores: count("cpu cores"),
            threads: count("siblings"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuArchitecture {
    pub microarchitecture: Option<String>,
    pub process: Option<String>,
    pub released: Option<String>,
    pub x86_64_level: Option<String>,
}

impl CpuArchitecture {
    fn current() -> Self {
        command::blocking::output(INXI_PROGRAM, &INXI_CPU_REPORT_ARGS)
            .ok()
            .and_then(|report| serde_json::from_str(&report).ok())
            .map(|report| Self::from_inxi_report(&report))
            .unwrap_or_default()
    }

    fn from_inxi_report(report: &Value) -> Self {
        let field = |name| inxi_field(report, name);
        CpuArchitecture {
            microarchitecture: field("arch"),
            process: field("process"),
            released: field("built"),
            x86_64_level: field("level"),
        }
    }
}

fn inxi_field(report: &Value, name: &str) -> Option<String> {
    match report {
        Value::Array(entries) => entries.iter().find_map(|entry| inxi_field(entry, name)),
        Value::Object(fields) => fields.iter().find_map(|(key, value)| {
            match key.rsplit_once('#').map(|(_, field_name)| field_name) {
                Some(field_name) if field_name == name => inxi_scalar(value),
                _ => inxi_field(value, name),
            }
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

fn inxi_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CacheKind {
    Data,
    Instruction,
    Unified,
}

impl CacheKind {
    fn from_sysfs(kind: &str) -> Option<Self> {
        match kind {
            "Data" => Some(CacheKind::Data),
            "Instruction" => Some(CacheKind::Instruction),
            "Unified" => Some(CacheKind::Unified),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CachePosition {
    pub level: u8,
    pub kind: CacheKind,
}

impl Display for CachePosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let level = self.level;
        match self.kind {
            CacheKind::Data => write!(f, "L{level} data"),
            CacheKind::Instruction => write!(f, "L{level} instruction"),
            CacheKind::Unified => write!(f, "L{level}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheInstance {
    position: CachePosition,
    size: ByteSize,
    shared_cpus: String,
}

impl CacheInstance {
    fn read(index: &Path) -> Option<Self> {
        let text = |name| Some(fs::read_to_string(index.join(name)).ok()?.trim().to_owned());
        let number = |name| text(name)?.parse::<u64>().ok();
        Some(CacheInstance {
            position: CachePosition {
                level: text("level")?.parse().ok()?,
                kind: CacheKind::from_sysfs(&text("type")?)?,
            },
            size: ByteSize::b(
                number("ways_of_associativity")?
                    * number("number_of_sets")?
                    * number("coherency_line_size")?
                    * number("physical_line_partition")?,
            ),
            shared_cpus: text("shared_cpu_list")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheInstances {
    pub size: ByteSize,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLevel {
    pub position: CachePosition,
    pub instances: Vec<CacheInstances>,
}

impl CacheLevel {
    fn of_instances(instances: &[CacheInstance]) -> Self {
        CacheLevel {
            position: instances[0].position,
            instances: instances
                .chunk_by(|a, b| a.size == b.size)
                .map(|same_size| CacheInstances {
                    size: same_size[0].size,
                    count: same_size.len(),
                })
                .collect(),
        }
    }
}

fn cache_levels() -> Vec<CacheLevel> {
    let instances = fs::read_dir(CPU_DEVICES)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|cpu| fs::read_dir(cpu.path().join("cache")).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|index| CacheInstance::read(&index.path()));
    cache_levels_of(instances)
}

fn cache_levels_of(instances: impl IntoIterator<Item = CacheInstance>) -> Vec<CacheLevel> {
    let mut distinct: Vec<CacheInstance> = instances
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    distinct.sort_by(|a, b| a.position.cmp(&b.position).then(b.size.cmp(&a.size)));
    distinct
        .chunk_by(|a, b| a.position == b.position)
        .map(CacheLevel::of_instances)
        .collect()
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

    const RAPHAEL_CPUINFO_CORE: &str = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: \
                                        AMD Ryzen 9 7950X3D 16-Core Processor\nmicrocode\t: \
                                        0xa60120c\nsiblings\t: 32\ncpu cores\t: 16\n";

    const RAPHAEL_INXI_REPORT: &str = r#"[{"000#1#0#CPU":[{"004#1#2#arch":"Zen 4","008#0#2#built":"2022+","009#0#2#process":"TSMC n5 (5nm)","006#1#2#level":"v4","002#0#2#bits":64},{"018#1#3#cores":16}]}]"#;

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
    fn cpuinfo_yields_name_microcode_and_topology() {
        assert_eq!(
            CpuIdentity::parse(RAPHAEL_CPUINFO_CORE),
            CpuIdentity {
                name: Some("AMD Ryzen 9 7950X3D 16-Core Processor".to_owned()),
                microcode: Some("0xa60120c".to_owned()),
            }
        );
        assert_eq!(
            CpuCoreCounts::parse(RAPHAEL_CPUINFO_CORE),
            CpuCoreCounts {
                cores: Some(16),
                threads: Some(32),
            }
        );
    }

    #[test]
    fn inxi_report_fields_are_found_by_name() {
        let report = serde_json::from_str(RAPHAEL_INXI_REPORT).unwrap();
        assert_eq!(
            CpuArchitecture::from_inxi_report(&report),
            CpuArchitecture {
                microarchitecture: Some("Zen 4".to_owned()),
                process: Some("TSMC n5 (5nm)".to_owned()),
                released: Some("2022+".to_owned()),
                x86_64_level: Some("v4".to_owned()),
            }
        );
    }

    #[test]
    fn shared_caches_are_counted_once_and_grouped_by_size() {
        let unified = |level, size, shared_cpus: &str| CacheInstance {
            position: CachePosition {
                level,
                kind: CacheKind::Unified,
            },
            size,
            shared_cpus: shared_cpus.to_owned(),
        };
        let instances = [
            unified(2, ByteSize::mib(1), "0,16"),
            unified(2, ByteSize::mib(1), "1,17"),
            unified(3, ByteSize::mib(32), "8-15,24-31"),
            unified(2, ByteSize::mib(1), "0,16"),
            unified(3, ByteSize::mib(96), "0-7,16-23"),
            unified(3, ByteSize::mib(96), "0-7,16-23"),
        ];
        assert_eq!(
            cache_levels_of(instances),
            vec![
                CacheLevel {
                    position: CachePosition {
                        level: 2,
                        kind: CacheKind::Unified,
                    },
                    instances: vec![CacheInstances {
                        size: ByteSize::mib(1),
                        count: 2,
                    }],
                },
                CacheLevel {
                    position: CachePosition {
                        level: 3,
                        kind: CacheKind::Unified,
                    },
                    instances: vec![
                        CacheInstances {
                            size: ByteSize::mib(96),
                            count: 1,
                        },
                        CacheInstances {
                            size: ByteSize::mib(32),
                            count: 1,
                        },
                    ],
                },
            ]
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
