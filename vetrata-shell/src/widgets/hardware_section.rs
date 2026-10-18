use vetrata_sources::hardware::{
    Adapter, CpuModel, DriveIdentity, MemoryModules, ModemIdentity, PowerSupplyIdentity,
};

use crate::widgets::tooltip_body::{Cell, KeyValue, Section};

pub trait HardwareEntries {
    fn hardware_entries(&self) -> Vec<KeyValue>;
}

impl Section {
    pub fn hardware_identity(entries: impl IntoIterator<Item = KeyValue>) -> Self {
        Section::dictionary(entries).titled("Hardware")
    }
}

impl From<&CpuModel> for Section {
    fn from(model: &CpuModel) -> Self {
        let identity = [
            ("Model", &model.identity.name),
            ("Microarchitecture", &model.architecture.microarchitecture),
            ("Process", &model.architecture.process),
            ("Released", &model.architecture.released),
            ("x86-64 level", &model.architecture.x86_64_level),
            ("Microcode", &model.identity.microcode),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)));
        let counts = [
            model
                .topology
                .cores
                .map(|count| KeyValue::new("Cores", count.to_string())),
            model
                .topology
                .threads
                .map(|count| KeyValue::new("Threads", count.to_string())),
        ]
        .into_iter()
        .flatten();
        let caches = model.caches.iter().map(|cache| {
            let sizes = cache
                .instances
                .iter()
                .map(|same_size| {
                    if same_size.count == 1 {
                        same_size.size.display().iec().to_string()
                    } else {
                        format!(
                            "{} each ×{}",
                            same_size.size.display().iec(),
                            same_size.count
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(" + ");
            KeyValue::new(cache.position.to_string(), sizes)
        });
        Section::dictionary(identity.chain(counts).chain(caches)).titled("Processor")
    }
}

impl HardwareEntries for MemoryModules {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        self.0
            .iter()
            .flat_map(|group| {
                let module = &group.module;
                let speed = module
                    .speed_mts
                    .map(|speed| KeyValue::new("Speed", format!("{speed} MT/s")));
                [
                    ("Manufacturer", &module.manufacturer),
                    ("Part number", &module.part_number),
                    ("Type", &module.kind),
                ]
                .into_iter()
                .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
                .chain(speed)
                .chain([
                    KeyValue::new("Module size", module.size.display().iec().to_string()),
                    KeyValue::new("Modules", group.count.to_string()),
                ])
            })
            .collect()
    }
}

impl HardwareEntries for DriveIdentity {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        [
            ("Model", &self.model),
            ("Serial", &self.serial),
            ("Firmware", &self.firmware),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
        .collect()
    }
}

impl HardwareEntries for PowerSupplyIdentity {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        [
            ("Manufacturer", &self.manufacturer),
            ("Model", &self.model),
            ("Serial", &self.serial),
            ("Technology", &self.technology),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
        .collect()
    }
}

impl HardwareEntries for ModemIdentity {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        [
            ("Manufacturer", &self.manufacturer),
            ("Model", &self.model),
            ("Firmware", &self.revision),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
        .chain(self.adapter.iter().flat_map(adapter_bus_entries))
        .collect()
    }
}

impl HardwareEntries for Adapter {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        [("Vendor", &self.vendor), ("Model", &self.model)]
            .into_iter()
            .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
            .chain(adapter_bus_entries(self))
            .collect()
    }
}

fn adapter_bus_entries(adapter: &Adapter) -> impl Iterator<Item = KeyValue> + '_ {
    let driver = adapter
        .driver
        .as_deref()
        .map(|driver| KeyValue::new("Driver", driver));
    [KeyValue::new(
        adapter.bus_id.bus_name(),
        adapter.bus_id.to_string(),
    )]
    .into_iter()
    .chain(driver)
    .chain([KeyValue::new(
        "Lookup",
        Cell::Link {
            label: format!("{} id database", adapter.bus_id.bus_name()),
            url: adapter.bus_id.lookup_url(),
        },
    )])
}
