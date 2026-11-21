use sottomano_desktop_sources::{
    battery_power_profiles::upower_battery_state::Battery,
    cpu_memory_compute::pci_gpu_samples::{GpuModel, GpuNames},
    disk_storage::drive_temperature_health::DriveHealth,
    host_machine_identity::udev_hardware_identity::{
        Adapter, CpuCoreCounts, CpuModel, DeviceIdentity, HardwareText, KernelDriver,
        ParentPciVendor,
    },
    network_connectivity::{
        interface_bus_attachment::AdapterIdentity, modem_manager_mobile::Mobile,
    },
};

use crate::{
    glyph_icons::simple_icons::SimpleIcon,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
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
        let vendor_logo = model.identity.vendor.vendor_logo();
        let named_model = match &model.identity.name {
            HardwareText::Reported(name) => Some(LabeledValue::logo_prefixed(
                "Model",
                vendor_logo,
                name.as_str(),
            )),
            HardwareText::Unreported => None,
        };
        let identity = [
            ("Microarchitecture", &model.architecture.microarchitecture),
            ("Process", &model.architecture.process),
            ("Released", &model.architecture.released),
            ("x86-64 level", &model.architecture.x86_64_level),
            ("Microcode", &model.identity.microcode),
        ]
        .into_iter()
        .filter_map(|(key, value)| reported_entry(key, value));
        let identity = named_model.into_iter().chain(identity);
        let counts = match model.topology {
            CpuCoreCounts::Counted { cores, threads } => vec![
                LabeledValue::new("Cores", Quantity::count(cores)),
                LabeledValue::new("Threads", Quantity::count(threads)),
            ],
            CpuCoreCounts::UnlistedInCpuinfo => Vec::new(),
        };
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

fn reported_entry(key: &'static str, value: &HardwareText) -> Option<LabeledValue> {
    match value {
        HardwareText::Reported(text) => Some(LabeledValue::new(key, text.as_str())),
        HardwareText::Unreported => None,
    }
}

fn device_identity_entries(
    identity: &DeviceIdentity,
    vendor_logo: Option<SimpleIcon>,
) -> impl Iterator<Item = LabeledValue> + '_ {
    let (named_vendor, model_logo) = match &identity.vendor {
        HardwareText::Reported(vendor) => (
            Some(LabeledValue::logo_prefixed(
                "Vendor",
                vendor_logo,
                vendor.as_str(),
            )),
            None,
        ),
        HardwareText::Unreported => (None, vendor_logo),
    };
    let named_model = match &identity.model {
        HardwareText::Reported(model) => Some(LabeledValue::logo_prefixed(
            "Model",
            model_logo,
            model.as_str(),
        )),
        HardwareText::Unreported => None,
    };
    named_vendor.into_iter().chain(named_model).chain(
        [
            ("Serial", &identity.serial),
            ("Firmware", &identity.revision),
        ]
        .into_iter()
        .filter_map(|(key, value)| reported_entry(key, value)),
    )
}

impl From<&DriveHealth> for Section {
    fn from(drive: &DriveHealth) -> Self {
        let vendor_logo = match drive.pci_vendor {
            ParentPciVendor::Identified { vendor_code } => {
                PciVendorId { vendor_code }.vendor_logo()
            }
            ParentPciVendor::Unidentified => None,
        };
        Section::hardware_identity(device_identity_entries(&drive.identity, vendor_logo))
    }
}

impl From<&Battery> for Section {
    fn from(battery: &Battery) -> Self {
        let technology = reported_entry("Technology", &battery.technology);
        Section::hardware_identity(
            device_identity_entries(&battery.identity, None).chain(technology),
        )
    }
}

impl From<&Mobile> for Section {
    fn from(modem: &Mobile) -> Self {
        Section::hardware_identity(device_identity_entries(&modem.identity, None).chain(
            match &modem.adapter {
                AdapterIdentity::Identified(adapter) => adapter_bus_entries(adapter).collect(),
                AdapterIdentity::Virtual => Vec::new(),
            },
        ))
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
    let driver = match &adapter.driver {
        KernelDriver::Bound(driver) => Some(LabeledValue::new("Driver", driver.as_str())),
        KernelDriver::Unbound => None,
    };
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
