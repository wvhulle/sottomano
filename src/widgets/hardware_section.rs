use crate::{
    data::source::hardware::{
        Adapter, CacheLevel, CpuModel, DriveIdentity, MemoryModuleGroup, MemoryModules,
        ModemIdentity, PowerSupplyIdentity,
    },
    widgets::tooltip_body::{Cell, KeyValue, Section},
};

pub trait HardwareEntries {
    fn hardware_entries(&self) -> Vec<KeyValue>;

    fn hardware_section(&self) -> Section {
        Section::dictionary(self.hardware_entries()).titled("Hardware")
    }
}

impl<H: HardwareEntries> HardwareEntries for &H {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        (*self).hardware_entries()
    }
}

impl<H: HardwareEntries> HardwareEntries for Option<H> {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        self.as_ref().map(H::hardware_entries).unwrap_or_default()
    }
}

impl CpuModel {
    pub fn hardware_sections(&self) -> [Section; 3] {
        let topology = &self.topology;
        [
            Section::dictionary(
                [
                    ("Model", &self.identity.name),
                    ("Microarchitecture", &self.architecture.microarchitecture),
                    ("Process", &self.architecture.process),
                    ("Released", &self.architecture.released),
                    ("x86-64 level", &self.architecture.x86_64_level),
                    ("Microcode", &self.identity.microcode),
                ]
                .into_iter()
                .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?))),
            )
            .titled("Processor"),
            Section::dictionary(
                [
                    topology
                        .cores
                        .map(|count| KeyValue::new("Cores", count.to_string())),
                    topology
                        .threads
                        .map(|count| KeyValue::new("Threads", count.to_string())),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Topology"),
            Section::dictionary(self.caches.0.iter().map(CacheLevel::key_value)).titled("Cache"),
        ]
    }
}

impl CacheLevel {
    fn key_value(&self) -> KeyValue {
        let instances = self
            .instances
            .iter()
            .map(|same_size| format!("{} × {}", same_size.count, same_size.size.display().iec()))
            .collect::<Vec<_>>()
            .join(" + ");
        KeyValue::new(self.position.to_string(), instances)
    }
}

impl HardwareEntries for MemoryModules {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        self.0.iter().flat_map(MemoryModuleGroup::entries).collect()
    }
}

impl MemoryModuleGroup {
    fn entries(&self) -> impl Iterator<Item = KeyValue> + '_ {
        let module = &self.module;
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
            KeyValue::new("Modules", self.count.to_string()),
        ])
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
        .chain(self.adapter.iter().flat_map(Adapter::bus_entries))
        .collect()
    }
}

impl HardwareEntries for Adapter {
    fn hardware_entries(&self) -> Vec<KeyValue> {
        [("Vendor", &self.vendor), ("Model", &self.model)]
            .into_iter()
            .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
            .chain(self.bus_entries())
            .collect()
    }
}

impl Adapter {
    fn bus_entries(&self) -> impl Iterator<Item = KeyValue> + '_ {
        let driver = self
            .driver
            .as_deref()
            .map(|driver| KeyValue::new("Driver", driver));
        [KeyValue::new(
            self.bus_id.bus_name(),
            self.bus_id.to_string(),
        )]
        .into_iter()
        .chain(driver)
        .chain([KeyValue::new(
            "Lookup",
            Cell::Link(self.bus_id.lookup_url()),
        )])
    }
}
