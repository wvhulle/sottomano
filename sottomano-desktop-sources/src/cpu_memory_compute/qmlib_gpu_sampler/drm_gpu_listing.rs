use std::collections::HashMap;

use qmlib::drm_devices::DrmDevices;

use crate::cpu_memory_compute::pci_gpu_samples::{
    GpuModel, GpuNames, GpuPowerLimit, GpuTemperatureLimits, GpuVendor, PciBusId,
};

pub(super) struct ListedGpu {
    pub(super) bus: PciBusId,
    pub(super) model: GpuModel,
}

pub(super) fn listed_gpus() -> Vec<ListedGpu> {
    let devices = match DrmDevices::find_devices(&Vec::new(), &HashMap::new()) {
        Ok(devices) => devices,
        Err(failure) => {
            tracing::warn!("gpu discovery: {failure:#}");
            return Vec::new();
        }
    };
    devices
        .devices()
        .into_iter()
        .filter_map(|bus| devices.device_info(bus))
        .filter(|device| device.has_driver())
        .filter_map(|device| {
            Some(ListedGpu {
                bus: PciBusId {
                    bus_address: device.pci_dev.clone(),
                },
                model: GpuModel {
                    vendor: gpu_vendor_of_pci_id(&device.vendor_id)?,
                    names: GpuNames::split_database_name(&device.device),
                    driver: device.drv_name.clone(),
                    temperature_limits: GpuTemperatureLimits::default(),
                    power_limit: GpuPowerLimit::NotExposed,
                },
            })
        })
        .collect()
}

pub(super) fn gpu_vendor_of_pci_id(vendor_id: &str) -> Option<GpuVendor> {
    let vendor_number = u16::from_str_radix(vendor_id, 16)
        .inspect_err(|parse_failure| {
            tracing::warn!("gpu discovery: invalid pci vendor id {vendor_id:?}: {parse_failure}");
        })
        .ok()?;
    match vendor_number {
        0x10de => Some(GpuVendor::Nvidia),
        0x8086 => Some(GpuVendor::Intel),
        0x1002 => Some(GpuVendor::Amd),
        _ => None,
    }
}
