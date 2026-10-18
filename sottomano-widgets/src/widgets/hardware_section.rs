use sottomano_sources::{
    gpu::GpuModel,
    hardware::{
        Adapter, CpuModel, DriveIdentity, MemoryModules, ModemIdentity, PowerSupplyIdentity,
    },
};

use crate::widgets::tooltip_body::{Cell, KeyValue, Section};

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

impl From<&MemoryModules> for Section {
    fn from(modules: &MemoryModules) -> Self {
        Section::hardware_identity(modules.0.iter().flat_map(|group| {
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
        }))
    }
}

impl From<&DriveIdentity> for Section {
    fn from(drive: &DriveIdentity) -> Self {
        Section::hardware_identity(
            [
                ("Model", &drive.model),
                ("Serial", &drive.serial),
                ("Firmware", &drive.firmware),
            ]
            .into_iter()
            .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?))),
        )
    }
}

impl From<&PowerSupplyIdentity> for Section {
    fn from(supply: &PowerSupplyIdentity) -> Self {
        Section::hardware_identity(
            [
                ("Manufacturer", &supply.manufacturer),
                ("Model", &supply.model),
                ("Serial", &supply.serial),
                ("Technology", &supply.technology),
            ]
            .into_iter()
            .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?))),
        )
    }
}

impl From<&ModemIdentity> for Section {
    fn from(modem: &ModemIdentity) -> Self {
        Section::hardware_identity(
            [
                ("Manufacturer", &modem.manufacturer),
                ("Model", &modem.model),
                ("Firmware", &modem.revision),
            ]
            .into_iter()
            .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
            .chain(modem.adapter.iter().flat_map(adapter_bus_entries)),
        )
    }
}

impl From<&Adapter> for Section {
    fn from(adapter: &Adapter) -> Self {
        Section::hardware_identity(
            [("Vendor", &adapter.vendor), ("Model", &adapter.model)]
                .into_iter()
                .filter_map(|(key, value)| Some(KeyValue::new(key, value.as_deref()?)))
                .chain(adapter_bus_entries(adapter)),
        )
    }
}

impl From<&GpuModel> for Section {
    fn from(model: &GpuModel) -> Self {
        Section::hardware_identity([
            KeyValue::new("Model", model.name.as_str()),
            KeyValue::new("Driver", model.driver_version.as_str()),
        ])
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
