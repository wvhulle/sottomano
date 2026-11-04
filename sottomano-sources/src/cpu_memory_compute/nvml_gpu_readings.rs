use std::{
    cmp::Reverse,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::Arc,
};

use bytesize::ByteSize;
use nvml_wrapper::{
    Device, Nvml,
    enum_wrappers::device::{Clock, PerformanceState, TemperatureSensor, TemperatureThreshold},
    enums::device::UsedGpuMemory,
    error::NvmlError,
};
use procfs::process::Process;
use tokio::task::spawn_blocking;
use uom::si::{
    f64::{Frequency, Power, Ratio, ThermodynamicTemperature},
    frequency::megahertz,
    power::milliwatt,
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use super::pci_gpu_readings::{
    DiscoveredGpu, GpuClocks, GpuHandle, GpuMemory, GpuModel, GpuNames, GpuProcess, GpuProcesses,
    GpuSnapshot, GpuTemperatureLimits, GpuVendor, PciBusId, VramUsage,
};
use crate::{
    blocking_background_work::{Panicked, blocking_poll},
    host_machine_identity::udev_hardware_identity::Adapter,
};

const NVIDIA_PCI_DRIVER: &str = "/sys/bus/pci/drivers/nvidia";
const PCI_DEVICES: &str = "/sys/bus/pci/devices";
const NVIDIA_MODULE_VERSION: &str = "/sys/module/nvidia/version";

pub(super) async fn discover_nvidia_gpus() -> Vec<DiscoveredGpu> {
    spawn_blocking(|| {
        let mut buses: Vec<PciBusId> = fs::read_dir(NVIDIA_PCI_DRIVER)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().join("vendor").exists())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .map(PciBusId)
            .collect();
        if buses.is_empty() {
            return Vec::new();
        }
        buses.sort();
        let nvml = Nvml::init().map(Arc::new).map_err(Error::from);
        buses
            .into_iter()
            .map(|bus| DiscoveredGpu {
                model: nvml
                    .as_ref()
                    .ok()
                    .and_then(|nvml| read_model(nvml, &bus).ok())
                    .unwrap_or_else(|| unmanaged_model(&bus)),
                handle: nvml
                    .as_ref()
                    .map(|nvml| GpuHandle::Nvml(NvmlDevice::new(bus.clone(), nvml)))
                    .map_err(|failure| failure.clone().into()),
                bus,
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct NvmlDevice {
    pub bus: PciBusId,
    nvml: Arc<Nvml>,
}

impl NvmlDevice {
    #[must_use]
    pub fn new(bus: PciBusId, nvml: &Arc<Nvml>) -> Self {
        Self {
            bus,
            nvml: Arc::clone(nvml),
        }
    }
}

impl PartialEq for NvmlDevice {
    fn eq(&self, other: &Self) -> bool {
        self.bus == other.bus
    }
}

impl Eq for NvmlDevice {}

impl Hash for NvmlDevice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bus.hash(state);
    }
}

fn celsius(degrees: u32) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(f64::from(degrees))
}

fn pci_database_names(bus: &PciBusId) -> Option<GpuNames> {
    let database_name = Adapter::of_device(Path::new(PCI_DEVICES).join(&bus.0))?.model?;
    Some(GpuNames::from_hardware_database(&database_name))
}

fn combined_gpu_names(database_names: Option<GpuNames>, nvml_product: String) -> GpuNames {
    match database_names {
        Some(names @ GpuNames::ChipWithProduct { .. }) => names,
        Some(GpuNames::Chip(chip)) => GpuNames::ChipWithProduct {
            chip,
            product: nvml_product,
        },
        Some(GpuNames::Product(_)) | None => GpuNames::Product(nvml_product),
    }
}

fn unmanaged_model(bus: &PciBusId) -> GpuModel {
    GpuModel {
        vendor: GpuVendor::Nvidia,
        names: pci_database_names(bus)
            .unwrap_or_else(|| GpuNames::Product("NVIDIA GPU".to_owned())),
        driver: "nvidia".to_owned(),
        temperature_limits: GpuTemperatureLimits::default(),
        power_limit: None,
    }
}

fn read_model(nvml: &Nvml, bus: &PciBusId) -> Result<GpuModel, NvmlError> {
    let device = nvml.device_by_pci_bus_id(bus.0.as_str())?;
    let product = device.name()?;
    Ok(GpuModel {
        vendor: GpuVendor::Nvidia,
        names: combined_gpu_names(pci_database_names(bus), product),
        driver: nvml.sys_driver_version()?,
        temperature_limits: GpuTemperatureLimits {
            target_max: device
                .temperature_threshold(TemperatureThreshold::GpuMax)
                .ok()
                .map(celsius),
            slowdown: device
                .temperature_threshold(TemperatureThreshold::Slowdown)
                .ok()
                .map(celsius),
        },
        power_limit: device.enforced_power_limit().ok().map(milliwatts),
    })
}

fn megahertz_clock(mhz: u32) -> Frequency {
    Frequency::new::<megahertz>(f64::from(mhz))
}

fn milliwatts(power: u32) -> Power {
    Power::new::<milliwatt>(f64::from(power))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    DriverMismatch { loaded_module: String },
    Management(String),
    Panicked,
}

impl From<Panicked> for Error {
    fn from(Panicked: Panicked) -> Self {
        Error::Panicked
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::DriverMismatch { loaded_module } => write!(
                f,
                "NVIDIA kernel module {loaded_module} is loaded, but the installed driver \
                 libraries differ; reboot to load the matching module"
            ),
            Error::Management(message) => write!(f, "NVML: {message}"),
            Error::Panicked => write!(f, "GPU read panicked"),
        }
    }
}

impl From<NvmlError> for Error {
    fn from(error: NvmlError) -> Self {
        if let NvmlError::LibRmVersionMismatch = error
            && let Ok(version) = fs::read_to_string(NVIDIA_MODULE_VERSION)
        {
            return Error::DriverMismatch {
                loaded_module: version.trim().to_owned(),
            };
        }
        Error::Management(error.to_string())
    }
}

pub(super) async fn read_snapshot(device: NvmlDevice) -> Result<GpuSnapshot, Error> {
    read_device(device, |device| {
        let memory = device.memory_info()?;
        Ok(GpuSnapshot {
            vendor: GpuVendor::Nvidia,
            load: Ratio::new::<percent>(f64::from(device.utilization_rates()?.gpu)),
            graphics_clock: device.clock_info(Clock::Graphics).ok().map(megahertz_clock),
            memory: GpuMemory::Dedicated(VramUsage {
                used: ByteSize::b(memory.used),
                total: ByteSize::b(memory.total),
            }),
            temperature: Some(celsius(device.temperature(TemperatureSensor::Gpu)?)),
            power_draw: Some(milliwatts(device.power_usage()?)),
        })
    })
    .await
}

pub(super) async fn read_clocks(device: NvmlDevice) -> Result<GpuClocks, Error> {
    read_device(device, |device| {
        Ok(GpuClocks {
            performance_state: device
                .performance_state()
                .ok()
                .filter(|state| *state != PerformanceState::Unknown)
                .map(|state| state.as_c()),
            graphics_clock_ceiling: device
                .max_clock_info(Clock::Graphics)
                .ok()
                .map(megahertz_clock),
            memory_clock: device.clock_info(Clock::Memory).ok().map(megahertz_clock),
            fan: device
                .fan_speed(0)
                .ok()
                .map(|speed| Ratio::new::<percent>(f64::from(speed))),
            throttle_reasons: Vec::new(),
        })
    })
    .await
}

pub(super) async fn read_processes(device: NvmlDevice) -> Result<GpuProcesses, Error> {
    read_device(device, |device| {
        Ok(GpuProcesses {
            processes: gpu_processes(device),
        })
    })
    .await
}

async fn read_device<T: Send + 'static>(
    device: NvmlDevice,
    read: fn(&Device) -> Result<T, Error>,
) -> Result<T, Error> {
    blocking_poll(move || read(&device.nvml.device_by_pci_bus_id(device.bus.0.as_str())?)).await
}

fn gpu_processes(device: &Device) -> Vec<GpuProcess> {
    let mut processes: Vec<GpuProcess> = [
        device.running_graphics_processes(),
        device.running_compute_processes(),
    ]
    .into_iter()
    .flatten()
    .flatten()
    .map(|info| GpuProcess {
        pid: info.pid,
        name: i32::try_from(info.pid)
            .ok()
            .and_then(|pid| Process::new(pid).and_then(|process| process.stat()).ok())
            .map(|stat| stat.comm),
        resident_memory: match info.used_gpu_memory {
            UsedGpuMemory::Used(bytes) => Some(ByteSize::b(bytes)),
            UsedGpuMemory::Unavailable => None,
        },
    })
    .collect();
    processes.sort_by_key(|process| process.pid);
    processes.dedup_by_key(|process| process.pid);
    processes.sort_by_key(|process| Reverse(process.resident_memory));
    processes
}

#[cfg(test)]
mod tests {
    use super::{GpuNames, combined_gpu_names};

    const NVML_PRODUCT: &str = "NVIDIA GeForce RTX 4090";

    fn combined_with_nvml_product(database_names: Option<GpuNames>) -> GpuNames {
        combined_gpu_names(database_names, NVML_PRODUCT.to_owned())
    }

    #[test]
    fn database_product_is_kept_over_the_branded_nvml_name() {
        let database_names = GpuNames::ChipWithProduct {
            chip: "AD102".to_owned(),
            product: "GeForce RTX 4090".to_owned(),
        };
        assert_eq!(
            combined_with_nvml_product(Some(database_names.clone())),
            database_names
        );
    }

    #[test]
    fn database_chip_without_single_product_takes_the_nvml_name() {
        assert_eq!(
            combined_with_nvml_product(Some(GpuNames::Chip("AD102".to_owned()))),
            GpuNames::ChipWithProduct {
                chip: "AD102".to_owned(),
                product: NVML_PRODUCT.to_owned(),
            }
        );
    }

    #[test]
    fn missing_database_entry_takes_the_nvml_name() {
        assert_eq!(
            combined_with_nvml_product(None),
            GpuNames::Product(NVML_PRODUCT.to_owned())
        );
    }
}
