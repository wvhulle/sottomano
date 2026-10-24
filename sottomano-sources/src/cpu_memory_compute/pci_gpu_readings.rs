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
    intel_drm_gpu_readings::{self, IntelDrmCard},
    nvml_gpu_readings::{self, NvmlDevice},
};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    blocking_background_work::blocking_poll,
    polled_delivery_stream,
    system_protocol_clients::sysfs_attribute_reads::{self, read_trimmed},
};

const PCI_DEVICES: &str = "/sys/bus/pci/devices";

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GpuHandle {
    Nvml(NvmlDevice),
    IntelDrm(IntelDrmCard),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredGpu {
    pub bus: PciBusId,
    pub model: Option<GpuModel>,
    pub handle: Result<GpuHandle, Error>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuDiscovery {
    pub gpus: Vec<DiscoveredGpu>,
}

impl ReadingSource for GpuDiscovery {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "gpu discovery";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::ReadOnce;

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(Self::bound_to_known_drivers())
            .map(|discovered| SourceDelivery::Reading(Ok(discovered)))
    }
}

impl GpuDiscovery {
    async fn bound_to_known_drivers() -> GpuDiscovery {
        let (nvidia, intel) = join(
            nvml_gpu_readings::discover_nvidia_gpus(),
            intel_drm_gpu_readings::discover_xe_gpus(),
        )
        .await;
        let mut gpus: Vec<DiscoveredGpu> = nvidia.into_iter().chain(intel).collect();
        gpus.sort_by(|first, second| first.bus.cmp(&second.bus));
        GpuDiscovery { gpus }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuRuntimePower {
    Awake,
    Asleep,
}

impl ReadingSource for GpuRuntimePower {
    type Error = sysfs_attribute_reads::Error;
    type StreamInputs = PciBusId;

    const NAME: &'static str = "gpu runtime power";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
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
    IntelDrm(sysfs_attribute_reads::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Nvml(error) => error.fmt(f),
            Error::IntelDrm(error) => error.fmt(f),
        }
    }
}

impl From<nvml_gpu_readings::Error> for Error {
    fn from(error: nvml_gpu_readings::Error) -> Self {
        Error::Nvml(error)
    }
}

impl From<sysfs_attribute_reads::Error> for Error {
    fn from(error: sysfs_attribute_reads::Error) -> Self {
        Error::IntelDrm(error)
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuMemory {
    Dedicated(VramUsage),
    SharedWithSystem { resident: ByteSize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuSnapshot {
    pub load: Ratio,
    pub graphics_clock: Option<Frequency>,
    pub memory: GpuMemory,
    pub temperature: Option<ThermodynamicTemperature>,
    pub power_draw: Option<Power>,
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

impl ReadingSource for GpuSnapshot {
    type Error = Error;
    type StreamInputs = GpuHandle;

    const NAME: &'static str = "gpu";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuSnapshot {
    async fn poll(handle: GpuHandle) -> Result<Self, Error> {
        match handle {
            GpuHandle::Nvml(device) => Ok(nvml_gpu_readings::read_snapshot(device).await?),
            GpuHandle::IntelDrm(card) => Ok(intel_drm_gpu_readings::read_snapshot(card).await?),
        }
    }
}

impl ReadingSource for GpuClocks {
    type Error = Error;
    type StreamInputs = GpuHandle;

    const NAME: &'static str = "gpu clocks";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuClocks {
    async fn poll(handle: GpuHandle) -> Result<Self, Error> {
        match handle {
            GpuHandle::Nvml(device) => Ok(nvml_gpu_readings::read_clocks(device).await?),
            GpuHandle::IntelDrm(card) => Ok(intel_drm_gpu_readings::read_clocks(card).await?),
        }
    }
}

impl ReadingSource for GpuProcesses {
    type Error = Error;
    type StreamInputs = GpuHandle;

    const NAME: &'static str = "gpu processes";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GpuProcesses {
    async fn poll(handle: GpuHandle) -> Result<Self, Error> {
        match handle {
            GpuHandle::Nvml(device) => Ok(nvml_gpu_readings::read_processes(device).await?),
            GpuHandle::IntelDrm(card) => Ok(intel_drm_gpu_readings::read_processes(card).await?),
        }
    }
}
