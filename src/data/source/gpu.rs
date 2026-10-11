use std::{
    cmp::Reverse,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    time::Duration,
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

use crate::data::source::Source;

const NVIDIA_PCI_DRIVER: &str = "/sys/bus/pci/drivers/nvidia";
const NVIDIA_MODULE_VERSION: &str = "/sys/module/nvidia/version";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PciBusId(pub String);

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuTemperatureLimits {
    pub target_max: Option<ThermodynamicTemperature>,
    pub slowdown: Option<ThermodynamicTemperature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuModel {
    pub name: String,
    pub driver_version: String,
    pub temperature_limits: GpuTemperatureLimits,
    pub power_limit: Option<Power>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredGpu {
    pub bus: PciBusId,
    pub model: Option<GpuModel>,
}

impl DiscoveredGpu {
    pub async fn bound_to_nvidia() -> Vec<DiscoveredGpu> {
        spawn_blocking(|| {
            let mut buses: Vec<PciBusId> = fs::read_dir(NVIDIA_PCI_DRIVER)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| entry.path().join("vendor").exists())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .map(PciBusId)
                .collect();
            buses.sort();
            let nvml = Nvml::init().ok();
            buses
                .into_iter()
                .map(|bus| DiscoveredGpu {
                    model: nvml.as_ref().and_then(|nvml| read_model(nvml, &bus).ok()),
                    bus,
                })
                .collect()
        })
        .await
        .unwrap_or_default()
    }
}

fn celsius(degrees: u32) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(f64::from(degrees))
}

fn read_model(nvml: &Nvml, bus: &PciBusId) -> Result<GpuModel, NvmlError> {
    let device = nvml.device_by_pci_bus_id(bus.0.as_str())?;
    Ok(GpuModel {
        name: device.name()?,
        driver_version: nvml.sys_driver_version()?,
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

fn milliwatts(power: u32) -> Power {
    Power::new::<milliwatt>(f64::from(power))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    DriverMismatch { loaded_module: String },
    Management(String),
    Panicked,
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

#[derive(Debug, Clone, PartialEq)]
pub struct GpuProcess {
    pub pid: u32,
    pub name: Option<String>,
    pub vram: Option<ByteSize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VramUsage {
    pub used: ByteSize,
    pub total: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuPowerDraw {
    pub draw: Power,
    pub limit: Option<Power>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuSnapshot {
    pub load: Ratio,
    pub vram: VramUsage,
    pub temperature: ThermodynamicTemperature,
    pub power_draw: Power,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuClocks {
    pub performance_state: Option<u32>,
    pub graphics_clock: Option<Frequency>,
    pub memory_clock: Option<Frequency>,
    pub fan: Option<Ratio>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuProcesses {
    pub processes: Vec<GpuProcess>,
}

impl Source for GpuSnapshot {
    type Error = Error;

    type Key = PciBusId;

    const NAME: &'static str = "gpu";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(bus: PciBusId) -> Result<Self, Self::Error> {
        read_device(bus, |device| {
            let memory = device.memory_info()?;
            Ok(GpuSnapshot {
                load: Ratio::new::<percent>(f64::from(device.utilization_rates()?.gpu)),
                vram: VramUsage {
                    used: ByteSize::b(memory.used),
                    total: ByteSize::b(memory.total),
                },
                temperature: celsius(device.temperature(TemperatureSensor::Gpu)?),
                power_draw: milliwatts(device.power_usage()?),
            })
        })
        .await
    }
}

impl Source for GpuClocks {
    type Error = Error;

    type Key = PciBusId;

    const NAME: &'static str = "gpu clocks";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(bus: PciBusId) -> Result<Self, Self::Error> {
        read_device(bus, |device| {
            let clock = |kind: Clock| {
                device
                    .clock_info(kind)
                    .ok()
                    .map(|mhz| Frequency::new::<megahertz>(f64::from(mhz)))
            };
            Ok(GpuClocks {
                performance_state: device
                    .performance_state()
                    .ok()
                    .filter(|state| *state != PerformanceState::Unknown)
                    .map(|state| state.as_c()),
                graphics_clock: clock(Clock::Graphics),
                memory_clock: clock(Clock::Memory),
                fan: device
                    .fan_speed(0)
                    .ok()
                    .map(|speed| Ratio::new::<percent>(f64::from(speed))),
            })
        })
        .await
    }
}

impl Source for GpuProcesses {
    type Error = Error;

    type Key = PciBusId;

    const NAME: &'static str = "gpu processes";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(bus: PciBusId) -> Result<Self, Self::Error> {
        read_device(bus, |device| {
            Ok(GpuProcesses {
                processes: gpu_processes(device),
            })
        })
        .await
    }
}

async fn read_device<T: Send + 'static>(
    bus: PciBusId,
    read: fn(&Device) -> Result<T, Error>,
) -> Result<T, Error> {
    spawn_blocking(move || {
        let nvml = Nvml::init()?;
        read(&nvml.device_by_pci_bus_id(bus.0.as_str())?)
    })
    .await
    .map_err(|_| Error::Panicked)?
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
        vram: match info.used_gpu_memory {
            UsedGpuMemory::Used(bytes) => Some(ByteSize::b(bytes)),
            UsedGpuMemory::Unavailable => None,
        },
    })
    .collect();
    processes.sort_by_key(|process| process.pid);
    processes.dedup_by_key(|process| process.pid);
    processes.sort_by_key(|process| Reverse(process.vram));
    processes
}
