use sottomano_sources::{
    cpu_memory_compute::pci_gpu_readings::{GpuModel, GpuNames},
    host_machine_identity::udev_hardware_identity::{
        Adapter, CpuModel, DriveIdentity, MemoryModules, ModemIdentity, PowerSupplyIdentity,
    },
};

use crate::widgets::{
    hover_card_body::Section,
    localized_quantities::{DisplayUnit, Quantity},
    value_cells::{LabeledValue, ValueCell},
    vendor_logo::{PciVendorId, VendorLogo},
};

impl Section {
    pub fn hardware_identity(entries: impl IntoIterator<Item = LabeledValue>) -> Self {
        Section::dictionary(entries).titled("Hardware")
    }
}

impl From<&CpuModel> for Section {
    fn from(model: &CpuModel) -> Self {
        let vendor_logo = model
            .identity
            .vendor
            .and_then(|vendor| vendor.vendor_logo());
        let named_model = model
            .identity
            .name
            .as_deref()
            .map(|name| LabeledValue::logo_prefixed("Model", vendor_logo, name));
        let identity = [
            ("Microarchitecture", &model.architecture.microarchitecture),
            ("Process", &model.architecture.process),
            ("Released", &model.architecture.released),
            ("x86-64 level", &model.architecture.x86_64_level),
            ("Microcode", &model.identity.microcode),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?)));
        let identity = named_model.into_iter().chain(identity);
        let counts = [
            model
                .topology
                .cores
                .map(|count| LabeledValue::new("Cores", Quantity::count(count))),
            model
                .topology
                .threads
                .map(|count| LabeledValue::new("Threads", Quantity::count(count))),
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
            LabeledValue::new(cache.position.to_string(), sizes)
        });
        Section::dictionary(identity.chain(counts).chain(caches)).titled("CPU model")
    }
}

impl From<&MemoryModules> for Section {
    fn from(modules: &MemoryModules) -> Self {
        Section::hardware_identity(modules.0.iter().flat_map(|group| {
            let module = &group.module;
            let speed = module.speed_mts.map(|speed| {
                LabeledValue::new(
                    "Speed",
                    Quantity::measured(f64::from(speed), DisplayUnit::MegatransfersPerSecond, 0),
                )
            });
            [
                ("Manufacturer", &module.manufacturer),
                ("Part number", &module.part_number),
                ("Type", &module.kind),
            ]
            .into_iter()
            .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?)))
            .chain(speed)
            .chain([
                LabeledValue::new("Module size", Quantity::size(module.size)),
                LabeledValue::new("Modules", Quantity::count(group.count)),
            ])
        }))
    }
}

impl From<&DriveIdentity> for Section {
    fn from(drive: &DriveIdentity) -> Self {
        let vendor_logo = drive
            .pci_vendor
            .and_then(|vendor| PciVendorId(vendor).vendor_logo());
        let named_model = drive
            .model
            .as_deref()
            .map(|model| LabeledValue::logo_prefixed("Model", vendor_logo, model));
        Section::hardware_identity(
            named_model.into_iter().chain(
                [("Serial", &drive.serial), ("Firmware", &drive.firmware)]
                    .into_iter()
                    .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?))),
            ),
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
            .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?))),
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
            .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?)))
            .chain(modem.adapter.iter().flat_map(adapter_bus_entries)),
        )
    }
}

impl From<&Adapter> for Section {
    fn from(adapter: &Adapter) -> Self {
        let named_vendor = adapter.vendor.as_deref().map(|vendor| {
            LabeledValue::logo_prefixed("Vendor", adapter.bus_id.vendor_logo(), vendor)
        });
        let model = adapter
            .model
            .as_deref()
            .map(|model| LabeledValue::new("Model", model));
        Section::hardware_identity(
            named_vendor
                .into_iter()
                .chain(model)
                .chain(adapter_bus_entries(adapter)),
        )
    }
}

impl From<&GpuModel> for Section {
    fn from(model: &GpuModel) -> Self {
        let logo = model.vendor.vendor_logo();
        let name_entries = match &model.names {
            GpuNames::Chip(chip) => vec![LabeledValue::logo_prefixed("Chip", logo, chip.as_str())],
            GpuNames::Product(product) => {
                vec![LabeledValue::logo_prefixed("Model", logo, product.as_str())]
            }
            GpuNames::ChipWithProduct { chip, product } => vec![
                LabeledValue::logo_prefixed("Model", logo, product.as_str()),
                LabeledValue::new("Chip", chip.as_str()),
            ],
        };
        Section::hardware_identity(
            name_entries
                .into_iter()
                .chain([LabeledValue::new("Driver", model.driver.as_str())]),
        )
    }
}

fn adapter_bus_entries(adapter: &Adapter) -> impl Iterator<Item = LabeledValue> + '_ {
    let driver = adapter
        .driver
        .as_deref()
        .map(|driver| LabeledValue::new("Driver", driver));
    [LabeledValue::new(
        adapter.bus_id.bus_name(),
        adapter.bus_id.to_string(),
    )]
    .into_iter()
    .chain(driver)
    .chain([LabeledValue::new(
        "Lookup",
        ValueCell::Link {
            label: format!("{} id database", adapter.bus_id.bus_name()),
            url: adapter.bus_id.lookup_url(),
        },
    )])
}
