use sottomano_desktop_sources::{
    battery_power_profiles::upower_battery_state::Battery,
    cpu_memory_compute::pci_gpu_samples::{GpuModel, GpuNames},
    disk_storage::drive_temperature_health::DriveHealth,
    host_machine_identity::udev_hardware_identity::{Adapter, CpuModel, DeviceIdentity},
    network_connectivity::modem_manager_mobile::Mobile,
};

use crate::{
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
        vendor_logo::{PciVendorId, VendorLogo},
    },
};

impl Section {
    pub fn hardware_identity(entries: impl IntoIterator<Item = LabeledValue>) -> Self {
        Section::new("Hardware", dictionary_table(entries))
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
        Section::new(
            "Identity",
            dictionary_table(identity.chain(counts).chain(caches)),
        )
    }
}

fn device_identity_entries(
    identity: &DeviceIdentity,
    vendor_logo: Option<SimpleIcon>,
) -> impl Iterator<Item = LabeledValue> + '_ {
    let named_vendor = identity
        .vendor
        .as_deref()
        .map(|vendor| LabeledValue::logo_prefixed("Vendor", vendor_logo, vendor));
    let model_logo = match identity.vendor {
        Some(_) => None,
        None => vendor_logo,
    };
    let named_model = identity
        .model
        .as_deref()
        .map(|model| LabeledValue::logo_prefixed("Model", model_logo, model));
    named_vendor.into_iter().chain(named_model).chain(
        [
            ("Serial", &identity.serial),
            ("Firmware", &identity.revision),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?))),
    )
}

impl From<&DriveHealth> for Section {
    fn from(drive: &DriveHealth) -> Self {
        let vendor_logo = drive
            .pci_vendor
            .and_then(|vendor| PciVendorId(vendor).vendor_logo());
        Section::hardware_identity(device_identity_entries(&drive.identity, vendor_logo))
    }
}

impl From<&Battery> for Section {
    fn from(battery: &Battery) -> Self {
        let technology = battery
            .technology
            .as_deref()
            .map(|technology| LabeledValue::new("Technology", technology));
        Section::hardware_identity(
            device_identity_entries(&battery.identity, None).chain(technology),
        )
    }
}

impl From<&Mobile> for Section {
    fn from(modem: &Mobile) -> Self {
        Section::hardware_identity(
            device_identity_entries(&modem.identity, None)
                .chain(modem.adapter.iter().flat_map(adapter_bus_entries)),
        )
    }
}

impl From<&Adapter> for Section {
    fn from(adapter: &Adapter) -> Self {
        Section::hardware_identity(
            device_identity_entries(&adapter.identity, adapter.bus_id.vendor_logo())
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
