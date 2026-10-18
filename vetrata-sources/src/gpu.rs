use std::{
    cmp::Reverse,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use bytesize::ByteSize;
use iced::futures::Stream;
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
    ratio::{percent, ratio},
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    DeliveryCadence, Panicked, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_stream,
};

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

#[derive(Debug, Clone)]
pub struct GpuDiscovery {
    pub gpus: Vec<DiscoveredGpu>,
    pub nvml: Result<Arc<Nvml>, Error>,
}

impl PartialEq for GpuDiscovery {
    fn eq(&self, other: &Self) -> bool {
        let same_management = match (&self.nvml, &other.nvml) {
            (Ok(mine), Ok(theirs)) => Arc::ptr_eq(mine, theirs),
            (Err(mine), Err(theirs)) => mine == theirs,
            _ => false,
        };
        self.gpus == other.gpus && same_management
    }
}

impl ReadingSource for GpuDiscovery {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "gpu discovery";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_hours(1));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuDiscovery {
    async fn poll(_key: ()) -> Result<Self, Error> {
        Ok(Self::bound_to_nvidia().await)
    }
}

impl GpuDiscovery {
    async fn bound_to_nvidia() -> GpuDiscovery {
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
            let nvml = Nvml::init().map(Arc::new).map_err(Error::from);
            GpuDiscovery {
                gpus: buses
                    .into_iter()
                    .map(|bus| DiscoveredGpu {
                        model: nvml
                            .as_ref()
                            .ok()
                            .and_then(|nvml| read_model(nvml, &bus).ok()),
                        bus,
                    })
                    .collect(),
                nvml,
            }
        })
        .await
        .unwrap_or_else(|_| GpuDiscovery {
            gpus: Vec::new(),
            nvml: Err(Error::Panicked),
        })
    }
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

impl VramUsage {
    #[must_use]
    pub fn fill(&self) -> Ratio {
        Ratio::new::<ratio>(self.used.as_u64() as f64 / self.total.as_u64() as f64)
    }
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

impl ReadingSource for GpuSnapshot {
    type Error = Error;
    type StreamInputs = NvmlDevice;

    const NAME: &'static str = "gpu";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuSnapshot {
    async fn poll(device: NvmlDevice) -> Result<Self, Error> {
        read_device(device, |device| {
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

impl ReadingSource for GpuClocks {
    type Error = Error;
    type StreamInputs = NvmlDevice;

    const NAME: &'static str = "gpu clocks";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuClocks {
    async fn poll(device: NvmlDevice) -> Result<Self, Error> {
        read_device(device, |device| {
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

impl ReadingSource for GpuProcesses {
    type Error = Error;
    type StreamInputs = NvmlDevice;

    const NAME: &'static str = "gpu processes";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuProcesses {
    async fn poll(device: NvmlDevice) -> Result<Self, Error> {
        read_device(device, |device| {
            Ok(GpuProcesses {
                processes: gpu_processes(device),
            })
        })
        .await
    }
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
