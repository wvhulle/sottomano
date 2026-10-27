use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::Path,
    time::Duration,
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, future::join, stream};
use uom::si::{
    f64::{Frequency, Power, Ratio, ThermodynamicTemperature},
    ratio::ratio,
};

use super::{
    memory_usage::Memory,
    nvml_gpu_readings::{self, NvmlDevice},
    qmlib_gpu_sampler::{self, DrmGpuWorker},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::sysfs_attribute_reads::{self, read_trimmed},
};

const PCI_DEVICES: &str = "/sys/bus/pci/devices";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PciBusId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct GpuModel {
    pub vendor: GpuVendor,
    pub names: GpuNames,
    pub driver: String,
    pub temperature_limits: GpuTemperatureLimits,
    pub power_limit: Option<Power>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuTemperatureLimits {
    pub target_max: Option<ThermodynamicTemperature>,
    pub slowdown: Option<ThermodynamicTemperature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GpuHandle {
    Nvml(NvmlDevice),
    Drm(DrmGpuWorker),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuNames {
    Chip(String),
    Product(String),
    ChipWithProduct { chip: String, product: String },
}

impl GpuNames {
    #[must_use]
    pub fn from_hardware_database(database_name: &str) -> Self {
        let Some((chip, bracketed)) = database_name.split_once(" [") else {
            return GpuNames::Chip(database_name.trim().to_owned());
        };
        let chip = chip.trim().to_owned();
        match bracketed.strip_suffix(']').map(str::trim) {
            Some(product) if !product.is_empty() && !product.contains('/') => {
                GpuNames::ChipWithProduct {
                    chip,
                    product: product.to_owned(),
                }
            }
            _ => GpuNames::Chip(chip),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredGpu {
    pub bus: PciBusId,
    pub model: GpuModel,
    pub handle: Result<GpuHandle, Error>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuDiscovery {
    pub gpus: Vec<DiscoveredGpu>,
}

impl SnapshotSource for GpuDiscovery {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "gpu discovery";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(Self::bound_to_known_drivers())
            .map(|discovered| SnapshotDelivery::single_read(Ok(discovered)))
    }
}

impl GpuDiscovery {
    async fn bound_to_known_drivers() -> GpuDiscovery {
        let (nvidia, drm) = join(
            nvml_gpu_readings::discover_nvidia_gpus(),
            qmlib_gpu_sampler::discover_drm_gpus(),
        )
        .await;
        let mut gpus: Vec<DiscoveredGpu> = nvidia.into_iter().chain(drm).collect();
        gpus.sort_by(|first, second| first.bus.cmp(&second.bus));
        GpuDiscovery { gpus }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuRuntimePower {
    Awake,
    Asleep,
}

impl SnapshotSource for GpuRuntimePower {
    type Error = sysfs_attribute_reads::Error;
    type StreamInputs = PciBusId;

    const NAME: &'static str = "gpu runtime power";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(3))
            .poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl GpuRuntimePower {
    async fn poll(bus: PciBusId) -> Result<Self, sysfs_attribute_reads::Error> {
        blocking_poll(move || {
            let status = Path::new(PCI_DEVICES)
                .join(&bus.0)
                .join("power/runtime_status");
            Ok(match read_trimmed(status).as_deref() {
                Some("suspended" | "suspending") => GpuRuntimePower::Asleep,
                _ => GpuRuntimePower::Awake,
            })
        })
        .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Nvml(nvml_gpu_readings::Error),
    Qmlib(String),
    DeviceVanished,
    WorkerStopped,
    Panicked,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Nvml(error) => error.fmt(f),
            Error::Qmlib(message) => write!(f, "GPU stats: {message}"),
            Error::DeviceVanished => f.write_str("GPU no longer listed by the DRM subsystem"),
            Error::WorkerStopped => f.write_str("GPU sampling thread stopped"),
            Error::Panicked => f.write_str("GPU read panicked"),
        }
    }
}

impl From<Panicked> for Error {
    fn from(Panicked: Panicked) -> Self {
        Error::Panicked
    }
}

impl From<nvml_gpu_readings::Error> for Error {
    fn from(error: nvml_gpu_readings::Error) -> Self {
        Error::Nvml(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuProcess {
    pub pid: u32,
    pub name: Option<String>,
    pub resident_memory: Option<ByteSize>,
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
pub enum GpuMemory {
    Dedicated(VramUsage),
    SharedWithSystem(SharedGpuMemory),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedGpuMemory {
    pub resident: ByteSize,
    pub allocated: ByteSize,
    pub system: Option<Memory>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuSnapshot {
    pub vendor: GpuVendor,
    pub load: Ratio,
    pub graphics_clock: Option<Frequency>,
    pub memory: GpuMemory,
    pub temperature: Option<ThermodynamicTemperature>,
    pub power_draw: Option<Power>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Intel,
    Amd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuThrottleReason {
    SustainedPowerLimit,
    BurstPowerLimit,
    PeakPowerLimit,
    ProcessorHot,
    RunningAverageThermalLimit,
    Thermal,
    VoltageRegulatorCurrent,
    VoltageRegulatorThermal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuClocks {
    pub performance_state: Option<u32>,
    pub graphics_clock_ceiling: Option<Frequency>,
    pub memory_clock: Option<Frequency>,
    pub fan: Option<Ratio>,
    pub throttle_reasons: Vec<GpuThrottleReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuProcesses {
    pub processes: Vec<GpuProcess>,
}

impl SnapshotSource for GpuSnapshot {
    type Error = Error;
    type StreamInputs = GpuHandle;

    const NAME: &'static str = "gpu";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(3))
            .poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl GpuSnapshot {
    async fn poll(handle: GpuHandle) -> Result<Self, Error> {
        match handle {
            GpuHandle::Nvml(device) => Ok(nvml_gpu_readings::read_snapshot(device).await?),
            GpuHandle::Drm(worker) => qmlib_gpu_sampler::read_snapshot(worker).await,
        }
    }
}

impl SnapshotSource for GpuClocks {
    type Error = Error;
    type StreamInputs = GpuHandle;

    const NAME: &'static str = "gpu clocks";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(3))
            .poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl GpuClocks {
    async fn poll(handle: GpuHandle) -> Result<Self, Error> {
        match handle {
            GpuHandle::Nvml(device) => Ok(nvml_gpu_readings::read_clocks(device).await?),
            GpuHandle::Drm(worker) => qmlib_gpu_sampler::read_clocks(worker).await,
        }
    }
}

impl SnapshotSource for GpuProcesses {
    type Error = Error;
    type StreamInputs = GpuHandle;

    const NAME: &'static str = "gpu processes";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(3))
            .poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl GpuProcesses {
    async fn poll(handle: GpuHandle) -> Result<Self, Error> {
        match handle {
            GpuHandle::Nvml(device) => Ok(nvml_gpu_readings::read_processes(device).await?),
            GpuHandle::Drm(worker) => qmlib_gpu_sampler::read_processes(worker).await,
        }
    }
}
