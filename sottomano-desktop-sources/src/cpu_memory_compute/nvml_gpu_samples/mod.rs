use std::{
    cmp::Reverse,
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

use super::pci_gpu_samples::{
    DiscoveredGpu, GpuClockSpeed, GpuClocks, GpuFanDuty, GpuHandle, GpuMemory, GpuModel, GpuNames,
    GpuPerformanceState, GpuPowerLimit, GpuPowerReading, GpuProcess, GpuProcessMemory,
    GpuProcessName, GpuProcesses, GpuSnapshot, GpuTemperatureLimits, GpuTemperatureReading,
    GpuTemperatureThreshold, GpuVendor, PciBusId, VramUsage,
};
use crate::{
    blocking_background_work::{Panicked, blocking_poll},
    host_machine_identity::udev_hardware_identity::{Adapter, HardwareText},
};

const NVIDIA_PCI_DRIVER: &str = "/sys/bus/pci/drivers/nvidia";
const PCI_DEVICES: &str = "/sys/bus/pci/devices";
const NVIDIA_MODULE_VERSION: &str = "/sys/module/nvidia/version";

pub(super) async fn discover_nvidia_gpus() -> Vec<DiscoveredGpu> {
    spawn_blocking(|| {
        let mut buses: Vec<PciBusId> = fs::read_dir(NVIDIA_PCI_DRIVER)
            .inspect_err(|listing_failure| {
                tracing::debug!("nvml: cannot list {NVIDIA_PCI_DRIVER}: {listing_failure}");
            })
            .into_iter()
            .flatten()
            .filter_map(|driver_entry| {
                driver_entry
                    .inspect_err(|entry_failure| {
                        tracing::debug!(
                            "nvml: skipping unreadable entry of {NVIDIA_PCI_DRIVER}: \
                             {entry_failure}"
                        );
                    })
                    .ok()
            })
            .filter(|entry| entry.path().join("vendor").exists())
            .filter_map(|entry| {
                entry
                    .file_name()
                    .into_string()
                    .inspect_err(|non_unicode_name| {
                        tracing::warn!("nvml: non-unicode pci bus id {non_unicode_name:?}");
                    })
                    .ok()
            })
            .map(|bus_address| PciBusId { bus_address })
            .collect();
        if buses.is_empty() {
            return Vec::new();
        }
        buses.sort();
        let nvml = Nvml::init().map(Arc::new).map_err(Error::from);
        buses
            .into_iter()
            .map(|bus| DiscoveredGpu {
                model: nvml.as_ref().map_or_else(
                    |_init_failure| unmanaged_model(&bus),
                    |nvml| {
                        read_model(nvml, &bus).unwrap_or_else(|model_failure| {
                            tracing::warn!(
                                "nvml: cannot read model of {}: {model_failure}",
                                bus.bus_address
                            );
                            unmanaged_model(&bus)
                        })
                    },
                ),
                handle: nvml
                    .as_ref()
                    .map(|nvml| GpuHandle::Nvml(NvmlDevice::new(bus.clone(), nvml)))
                    .map_err(|failure| failure.clone().into()),
                bus,
            })
            .collect()
    })
    .await
    .unwrap_or_else(|join_failure| {
        tracing::warn!("nvml: gpu discovery panicked: {join_failure}");
        Vec::new()
    })
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
    let HardwareText::Reported(database_name) =
        Adapter::read_sysfs_device(Path::new(PCI_DEVICES).join(&bus.bus_address))?
            .identity
            .model
    else {
        return None;
    };
    Some(GpuNames::split_database_name(&database_name))
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
        power_limit: GpuPowerLimit::NotExposed,
    }
}

fn read_model(nvml: &Nvml, bus: &PciBusId) -> Result<GpuModel, NvmlError> {
    let device = nvml.device_by_pci_bus_id(bus.bus_address.as_str())?;
    let product = device.name()?;
    Ok(GpuModel {
        vendor: GpuVendor::Nvidia,
        names: combined_gpu_names(pci_database_names(bus), product),
        driver: nvml.sys_driver_version()?,
        temperature_limits: GpuTemperatureLimits {
            target_max: optional_sample(
                device.temperature_threshold(TemperatureThreshold::GpuMax),
                "maximum temperature",
            )
            .map_or(GpuTemperatureThreshold::NotExposed, |threshold| {
                GpuTemperatureThreshold::Reported(celsius(threshold))
            }),
            slowdown: optional_sample(
                device.temperature_threshold(TemperatureThreshold::Slowdown),
                "slowdown temperature",
            )
            .map_or(GpuTemperatureThreshold::NotExposed, |threshold| {
                GpuTemperatureThreshold::Reported(celsius(threshold))
            }),
        },
        power_limit: optional_sample(device.enforced_power_limit(), "power limit")
            .map_or(GpuPowerLimit::NotExposed, |limit| {
                GpuPowerLimit::Enforced(milliwatts(limit))
            }),
    })
}

fn optional_sample<Sample>(sample: Result<Sample, NvmlError>, quantity: &str) -> Option<Sample> {
    sample
        .inspect_err(|nvml_failure| {
            if let NvmlError::NotSupported = nvml_failure {
                tracing::debug!("nvml: {quantity} not supported");
            } else {
                tracing::warn!("nvml: cannot read {quantity}: {nvml_failure}");
            }
        })
        .ok()
}

fn reported_clock(clock: Result<u32, NvmlError>, quantity: &str) -> GpuClockSpeed {
    optional_sample(clock, quantity).map_or(GpuClockSpeed::NotExposed, |clock_megahertz| {
        GpuClockSpeed::Reported(Frequency::new::<megahertz>(f64::from(clock_megahertz)))
    })
}

fn milliwatts(power: u32) -> Power {
    Power::new::<milliwatt>(f64::from(power))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error(
        "NVIDIA kernel module {loaded_module} is loaded, but the installed driver libraries \
         differ; reboot to load the matching module"
    )]
    DriverMismatch { loaded_module: String },
    #[error("NVML: {0}")]
    Management(String),
    #[error("GPU read panicked")]
    Panicked,
}

impl From<Panicked> for Error {
    fn from(Panicked: Panicked) -> Self {
        Error::Panicked
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
            graphics_clock: reported_clock(device.clock_info(Clock::Graphics), "graphics clock"),
            memory: GpuMemory::Dedicated(VramUsage {
                used: ByteSize::b(memory.used),
                total: ByteSize::b(memory.total),
            }),
            temperature: GpuTemperatureReading::Measured(celsius(
                device.temperature(TemperatureSensor::Gpu)?,
            )),
            power_draw: GpuPowerReading::Drawing(milliwatts(device.power_usage()?)),
        })
    })
    .await
}

pub(super) async fn read_clocks(device: NvmlDevice) -> Result<GpuClocks, Error> {
    read_device(device, |device| {
        Ok(GpuClocks {
            performance_state: optional_sample(device.performance_state(), "performance state")
                .filter(|state| *state != PerformanceState::Unknown)
                .map_or(GpuPerformanceState::NotExposed, |state| {
                    GpuPerformanceState::PowerState(state.as_c())
                }),
            graphics_clock_ceiling: reported_clock(
                device.max_clock_info(Clock::Graphics),
                "graphics clock ceiling",
            ),
            memory_clock: reported_clock(device.clock_info(Clock::Memory), "memory clock"),
            fan: optional_sample(device.fan_speed(0), "fan speed")
                .map_or(GpuFanDuty::NotExposed, |speed| {
                    GpuFanDuty::Spinning(Ratio::new::<percent>(f64::from(speed)))
                }),
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
    blocking_poll(move || {
        read(
            &device
                .nvml
                .device_by_pci_bus_id(device.bus.bus_address.as_str())?,
        )
    })
    .await
}

fn gpu_processes(device: &Device) -> Vec<GpuProcess> {
    let mut processes: Vec<GpuProcess> = [
        optional_sample(device.running_graphics_processes(), "graphics processes"),
        optional_sample(device.running_compute_processes(), "compute processes"),
    ]
    .into_iter()
    .flatten()
    .flatten()
    .map(|info| GpuProcess {
        pid: info.pid,
        name: i32::try_from(info.pid)
            .inspect_err(|conversion_failure| {
                tracing::warn!(
                    "nvml: process id {} out of range: {conversion_failure}",
                    info.pid
                );
            })
            .ok()
            .and_then(|pid| {
                Process::new(pid)
                    .and_then(|process| process.stat())
                    .inspect_err(|stat_failure| {
                        tracing::debug!(
                            "nvml: cannot read stat of gpu process {pid}: {stat_failure}"
                        );
                    })
                    .ok()
            })
            .map_or(GpuProcessName::Unreadable, |stat| {
                GpuProcessName::Command(stat.comm)
            }),
        resident_memory: match info.used_gpu_memory {
            UsedGpuMemory::Used(bytes) => GpuProcessMemory::Resident(ByteSize::b(bytes)),
            UsedGpuMemory::Unavailable => GpuProcessMemory::NotReported,
        },
    })
    .collect();
    processes.sort_by_key(|process| process.pid);
    processes.dedup_by_key(|process| process.pid);
    processes.sort_by_key(|process| Reverse(process.resident_memory));
    processes
}

#[cfg(test)]
mod nvml_gpu_samples_tests;
