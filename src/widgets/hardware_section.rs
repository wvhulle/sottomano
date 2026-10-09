use crate::{
    data::source::hardware::{
        Adapter, CacheLevel, CpuModel, DriveIdentity, MemoryModuleGroup, MemoryModules,
        ModemIdentity, PowerSupplyIdentity,
    },
    widgets::tooltip_body::{Cell, Row, Section},
};

pub trait HardwareRows {
    fn hardware_rows(&self) -> Vec<Row>;

    fn hardware_section(&self) -> Section {
        Section::titled("Hardware", &self.hardware_rows())
    }
}

impl<H: HardwareRows> HardwareRows for &H {
    fn hardware_rows(&self) -> Vec<Row> {
        (*self).hardware_rows()
    }
}

impl<H: HardwareRows> HardwareRows for Option<H> {
    fn hardware_rows(&self) -> Vec<Row> {
        self.as_ref().map(H::hardware_rows).unwrap_or_default()
    }
}

impl CpuModel {
    pub fn hardware_sections(&self) -> [Section; 3] {
        let caches: Vec<Row> = self.caches.0.iter().map(CacheLevel::row).collect();
        [
            Section::titled(
                "Processor",
                &[
                    ("Model", &self.identity.name),
                    ("Microarchitecture", &self.architecture.microarchitecture),
                    ("Process", &self.architecture.process),
                    ("Released", &self.architecture.released),
                    ("x86-64 level", &self.architecture.x86_64_level),
                    ("Microcode", &self.identity.microcode),
                ],
            ),
            Section::titled(
                "Topology",
                &[
                    ("Cores", self.topology.cores.map(|count| count.to_string())),
                    (
                        "Threads",
                        self.topology.threads.map(|count| count.to_string()),
                    ),
                ],
            ),
            Section::titled("Cache", &caches),
        ]
    }
}

impl CacheLevel {
    fn row(&self) -> Row {
        let instances = self
            .instances
            .iter()
            .map(|same_size| format!("{} × {}", same_size.count, same_size.size.display().iec()))
            .collect::<Vec<_>>()
            .join(" + ");
        (self.position.to_string(), instances).into()
    }
}

impl HardwareRows for MemoryModules {
    fn hardware_rows(&self) -> Vec<Row> {
        self.0.iter().flat_map(MemoryModuleGroup::rows).collect()
    }
}

impl MemoryModuleGroup {
    fn rows(&self) -> Vec<Row> {
        let module = &self.module;
        [
            ("Manufacturer", module.manufacturer.clone()),
            ("Part number", module.part_number.clone()),
            ("Type", module.kind.clone()),
            (
                "Speed",
                module.speed_mts.map(|speed| format!("{speed} MT/s")),
            ),
            ("Module size", Some(module.size.display().iec().to_string())),
            ("Modules", Some(self.count.to_string())),
        ]
        .map(Row::from)
        .into()
    }
}

impl HardwareRows for DriveIdentity {
    fn hardware_rows(&self) -> Vec<Row> {
        [
            ("Model", &self.model),
            ("Serial", &self.serial),
            ("Firmware", &self.firmware),
        ]
        .map(Row::from)
        .into()
    }
}

impl HardwareRows for PowerSupplyIdentity {
    fn hardware_rows(&self) -> Vec<Row> {
        [
            ("Manufacturer", &self.manufacturer),
            ("Model", &self.model),
            ("Serial", &self.serial),
            ("Technology", &self.technology),
        ]
        .map(Row::from)
        .into()
    }
}

impl HardwareRows for ModemIdentity {
    fn hardware_rows(&self) -> Vec<Row> {
        [
            [
                ("Manufacturer", &self.manufacturer),
                ("Model", &self.model),
                ("Firmware", &self.revision),
            ]
            .map(Row::from)
            .to_vec(),
            self.adapter
                .as_ref()
                .map(Adapter::bus_rows)
                .unwrap_or_default(),
        ]
        .concat()
    }
}

impl HardwareRows for Adapter {
    fn hardware_rows(&self) -> Vec<Row> {
        [
            [("Vendor", &self.vendor), ("Model", &self.model)]
                .map(Row::from)
                .to_vec(),
            self.bus_rows(),
        ]
        .concat()
    }
}

impl Adapter {
    fn bus_rows(&self) -> Vec<Row> {
        [
            (self.bus_id.bus_name(), Cell::from(self.bus_id.to_string())),
            ("Driver", Cell::from(&self.driver)),
            ("Lookup", Cell::Link(self.bus_id.lookup_url())),
        ]
        .map(Row::from)
        .into()
    }
}
