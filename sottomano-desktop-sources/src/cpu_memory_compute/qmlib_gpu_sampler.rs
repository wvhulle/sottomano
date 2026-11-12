use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use qmlib::{
    drm_clients::DrmClientMemInfo,
    drm_devices::{DrmDeviceInfo, DrmDeviceThrottleReasons, DrmDevices},
};
use tokio::sync::oneshot;
use uom::si::{
    f64::{Frequency, Power, Ratio, ThermodynamicTemperature},
    frequency::megahertz,
    power::watt,
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use super::{
    memory_usage::read_meminfo,
    pci_gpu_readings::{
        DiscoveredGpu, Error, GpuClocks, GpuHandle, GpuMemory, GpuModel, GpuNames, GpuProcess,
        GpuProcesses, GpuSnapshot, GpuTemperatureLimits, GpuThrottleReason, GpuVendor, PciBusId,
        SharedGpuMemory, VramUsage,
    },
};
use crate::blocking_background_work::blocking;

const SCAN_EVERY_PROCESS: &str = "";
const MINIMUM_REFRESH_GAP: Duration = Duration::from_secs(1);

enum SampleRequest {
    Snapshot(oneshot::Sender<Result<GpuSnapshot, Error>>),
    Clocks(oneshot::Sender<Result<GpuClocks, Error>>),
    Processes(oneshot::Sender<Result<GpuProcesses, Error>>),
}

#[derive(Clone)]
pub struct DrmGpuWorker {
    pub bus: PciBusId,
    requests: Sender<SampleRequest>,
}

impl Debug for DrmGpuWorker {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("DrmGpuWorker")
            .field("bus", &self.bus)
            .finish_non_exhaustive()
    }
}

impl PartialEq for DrmGpuWorker {
    fn eq(&self, other: &Self) -> bool {
        self.bus == other.bus
    }
}

impl Eq for DrmGpuWorker {}

impl Hash for DrmGpuWorker {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bus.hash(state);
    }
}

impl DrmGpuWorker {
    fn start(bus: PciBusId, vendor: GpuVendor) -> Result<Self, Error> {
        let (requests, incoming_requests) = mpsc::channel();
        let (opened_sender, opened_receiver) = mpsc::sync_channel(1);
        let worker_bus = bus.clone();
        thread::Builder::new()
            .name(format!("gpu {}", bus.0))
            .spawn(move || match GpuSampleWorker::open(worker_bus, vendor) {
                Ok(worker) => {
                    let _ = opened_sender.send(Ok(()));
                    worker.serve(&incoming_requests);
                }
                Err(failure) => {
                    let _ = opened_sender.send(Err(failure));
                }
            })
            .map_err(|failure| Error::Qmlib(failure.to_string()))?;
        opened_receiver
            .recv()
            .map_err(|_dropped_sender| Error::Panicked)??;
        Ok(DrmGpuWorker { bus, requests })
    }

    async fn request<T>(
        &self,
        wrap_reply: fn(oneshot::Sender<Result<T, Error>>) -> SampleRequest,
    ) -> Result<T, Error> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.requests
            .send(wrap_reply(reply_sender))
            .map_err(|_| Error::WorkerStopped)?;
        reply_receiver
            .await
            .map_err(|_dropped_reply| Error::WorkerStopped)?
    }
}

pub(super) async fn read_snapshot(worker: DrmGpuWorker) -> Result<GpuSnapshot, Error> {
    worker.request(SampleRequest::Snapshot).await
}

pub(super) async fn read_clocks(worker: DrmGpuWorker) -> Result<GpuClocks, Error> {
    worker.request(SampleRequest::Clocks).await
}

pub(super) async fn read_processes(worker: DrmGpuWorker) -> Result<GpuProcesses, Error> {
    worker.request(SampleRequest::Processes).await
}

pub(super) async fn discover_drm_gpus() -> Vec<DiscoveredGpu> {
    blocking("gpu discovery", || {
        listed_gpus()
            .into_iter()
            .map(|listed| DiscoveredGpu {
                handle: DrmGpuWorker::start(listed.bus.clone(), listed.model.vendor)
                    .map(GpuHandle::Drm),
                model: listed.model,
                bus: listed.bus,
            })
            .collect()
    })
    .await
}

struct ListedGpu {
    bus: PciBusId,
    model: GpuModel,
}

fn listed_gpus() -> Vec<ListedGpu> {
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
                bus: PciBusId(device.pci_dev.clone()),
                model: GpuModel {
                    vendor: gpu_vendor_of_pci_id(&device.vendor_id)?,
                    names: GpuNames::from_hardware_database(&device.device),
                    driver: device.drv_name.clone(),
                    temperature_limits: GpuTemperatureLimits::default(),
                    power_limit: None,
                },
            })
        })
        .collect()
}

fn gpu_vendor_of_pci_id(vendor_id: &str) -> Option<GpuVendor> {
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

fn qmlib_failure(failure: impl Display) -> Error {
    Error::Qmlib(format!("{failure:#}"))
}

struct GpuSampleWorker {
    bus: PciBusId,
    vendor: GpuVendor,
    devices: DrmDevices,
    last_refresh: Option<Instant>,
}

struct ClientSample {
    pid: u32,
    name: String,
    memory: DrmClientMemInfo,
}

impl GpuSampleWorker {
    fn open(bus: PciBusId, vendor: GpuVendor) -> Result<Self, Error> {
        let mut devices =
            catch_unwind(|| DrmDevices::find_devices(&vec![bus.0.as_str()], &HashMap::new()))
                .map_err(|_| Error::Panicked)?
                .map_err(qmlib_failure)?;
        devices
            .set_clients_pid_tree(SCAN_EVERY_PROCESS)
            .map_err(qmlib_failure)?;
        let worker = GpuSampleWorker {
            bus,
            vendor,
            devices,
            last_refresh: None,
        };
        worker.device()?;
        Ok(worker)
    }

    fn serve(mut self, incoming_requests: &Receiver<SampleRequest>) {
        for request in incoming_requests {
            let refreshed = self.refresh_when_stale();
            match request {
                SampleRequest::Snapshot(reply) => {
                    let _ = reply.send(refreshed.and_then(|()| self.snapshot()));
                }
                SampleRequest::Clocks(reply) => {
                    let _ = reply.send(refreshed.and_then(|()| self.clocks()));
                }
                SampleRequest::Processes(reply) => {
                    let _ = reply.send(refreshed.and_then(|()| self.processes()));
                }
            }
        }
    }

    fn refresh_when_stale(&mut self) -> Result<(), Error> {
        if self.last_refresh.is_some_and(|refreshed_at| {
            Instant::now()
                .checked_duration_since(refreshed_at)
                .expect("the last gpu refresh happened before now")
                < MINIMUM_REFRESH_GAP
        }) {
            return Ok(());
        }
        catch_unwind(AssertUnwindSafe(|| self.devices.refresh()))
            .map_err(|_| Error::Panicked)?
            .map_err(qmlib_failure)?;
        self.last_refresh = Some(Instant::now());
        Ok(())
    }

    fn device(&self) -> Result<&DrmDeviceInfo, Error> {
        self.devices
            .device_info(&self.bus.0)
            .ok_or(Error::DeviceVanished)
    }

    fn client_samples(device: &DrmDeviceInfo) -> Vec<ClientSample> {
        let Some(clients) = device.clients().and_then(|clients| clients.upgrade()) else {
            return Vec::new();
        };
        let clients = clients.borrow();
        clients
            .values()
            .map(|client| {
                let process = client.proc.borrow();
                ClientSample {
                    pid: process.pid,
                    name: process.comm.clone(),
                    memory: client.mem_info(),
                }
            })
            .collect()
    }

    fn snapshot(&self) -> Result<GpuSnapshot, Error> {
        let device = self.device()?;
        Ok(GpuSnapshot {
            vendor: self.vendor,
            load: busiest_engine_load(device),
            graphics_clock: device
                .freqs
                .first()
                .filter(|frequencies| frequencies.act_freq > 0)
                .map(|frequencies| megahertz_frequency(frequencies.act_freq)),
            memory: Self::gpu_memory(device),
            temperature: device
                .dev_type
                .is_discrete()
                .then(|| device.temps.first())
                .flatten()
                .map(|sensor| ThermodynamicTemperature::new::<degree_celsius>(sensor.temp)),
            power_draw: device
                .power
                .as_ref()
                .map(|power| Power::new::<watt>(power.gpu_cur_power)),
        })
    }

    fn gpu_memory(device: &DrmDeviceInfo) -> GpuMemory {
        if let Some(memory) = device
            .mem_info
            .as_ref()
            .filter(|memory| memory.vram_total > 0)
        {
            return GpuMemory::Dedicated(VramUsage {
                used: ByteSize::b(memory.vram_used),
                total: ByteSize::b(memory.vram_total),
            });
        }
        let clients = Self::client_samples(device);
        GpuMemory::SharedWithSystem(SharedGpuMemory {
            resident: clients
                .iter()
                .map(|client| ByteSize::b(client.memory.smem_rss))
                .sum(),
            allocated: clients
                .iter()
                .map(|client| ByteSize::b(client.memory.smem_used))
                .sum(),
            system: read_meminfo()
                .inspect_err(|meminfo_failure| {
                    tracing::warn!("gpu memory: cannot read system memory: {meminfo_failure}");
                })
                .ok(),
        })
    }

    fn clocks(&self) -> Result<GpuClocks, Error> {
        let device = self.device()?;
        let frequencies = device.freqs.first();
        Ok(GpuClocks {
            performance_state: None,
            memory_clock: None,
            fan: None,
            graphics_clock_ceiling: device
                .freq_limits
                .first()
                .map(|limits| limits.maximum)
                .or_else(|| frequencies.map(|frequencies| frequencies.max_freq))
                .filter(|maximum| *maximum > 0)
                .map(megahertz_frequency),
            throttle_reasons: frequencies
                .map(|frequencies| active_throttle_reasons(&frequencies.throttle_reasons))
                .unwrap_or_default(),
        })
    }

    fn processes(&self) -> Result<GpuProcesses, Error> {
        let device = self.device()?;
        let mut processes_by_pid: BTreeMap<u32, GpuProcess> = BTreeMap::new();
        for client in Self::client_samples(device) {
            let resident = ByteSize::b(client.memory.smem_used + client.memory.vram_used);
            let process = processes_by_pid
                .entry(client.pid)
                .or_insert_with(|| GpuProcess {
                    pid: client.pid,
                    name: Some(client.name),
                    resident_memory: Some(ByteSize::b(0)),
                });
            process.resident_memory = process
                .resident_memory
                .map(|accumulated| accumulated + resident);
        }
        let mut processes: Vec<GpuProcess> = processes_by_pid.into_values().collect();
        processes.sort_by_key(|process| Reverse(process.resident_memory));
        Ok(GpuProcesses { processes })
    }
}

fn megahertz_frequency(megahertz_value: u64) -> Frequency {
    Frequency::new::<megahertz>(megahertz_value as f64)
}

fn busiest_engine_load(device: &DrmDeviceInfo) -> Ratio {
    let busiest_percent = device
        .engines()
        .iter()
        .map(|engine| device.eng_utilization(engine))
        .fold(0.0, f64::max);
    Ratio::new::<percent>(busiest_percent.clamp(0.0, 100.0))
}

fn active_throttle_reasons(reasons: &DrmDeviceThrottleReasons) -> Vec<GpuThrottleReason> {
    if !reasons.status {
        return Vec::new();
    }
    [
        (reasons.pl1, GpuThrottleReason::SustainedPowerLimit),
        (reasons.pl2, GpuThrottleReason::BurstPowerLimit),
        (reasons.pl4, GpuThrottleReason::PeakPowerLimit),
        (reasons.prochot, GpuThrottleReason::ProcessorHot),
        (reasons.ratl, GpuThrottleReason::RunningAverageThermalLimit),
        (reasons.thermal, GpuThrottleReason::Thermal),
        (reasons.vr_tdc, GpuThrottleReason::VoltageRegulatorCurrent),
        (
            reasons.vr_thermalert,
            GpuThrottleReason::VoltageRegulatorThermal,
        ),
    ]
    .into_iter()
    .filter(|(active, _)| *active)
    .map(|(_, reason)| reason)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{GpuNames, GpuVendor, gpu_vendor_of_pci_id};

    #[test]
    fn reads_vendor_from_udev_hex_in_either_case() {
        assert_eq!(gpu_vendor_of_pci_id("10DE"), Some(GpuVendor::Nvidia));
        assert_eq!(gpu_vendor_of_pci_id("10de"), Some(GpuVendor::Nvidia));
        assert_eq!(gpu_vendor_of_pci_id("8086"), Some(GpuVendor::Intel));
        assert_eq!(gpu_vendor_of_pci_id("1002"), Some(GpuVendor::Amd));
        assert_eq!(gpu_vendor_of_pci_id("1AF4"), None);
    }

    #[test]
    fn splits_chip_from_single_product_name() {
        assert_eq!(
            GpuNames::from_hardware_database(
                "GB206GLM [RTX PRO 2000 Blackwell Generation Laptop GPU]"
            ),
            GpuNames::ChipWithProduct {
                chip: "GB206GLM".to_owned(),
                product: "RTX PRO 2000 Blackwell Generation Laptop GPU".to_owned(),
            }
        );
    }

    #[test]
    fn keeps_only_chip_when_products_are_alternatives() {
        assert_eq!(
            GpuNames::from_hardware_database("Arrow Lake-P [Arc Pro 130T/140T]"),
            GpuNames::Chip("Arrow Lake-P".to_owned())
        );
    }

    #[test]
    fn unbracketed_name_is_the_chip() {
        assert_eq!(
            GpuNames::from_hardware_database("Navi 31"),
            GpuNames::Chip("Navi 31".to_owned())
        );
    }

    #[test]
    fn preferred_name_is_the_product_when_known() {
        assert_eq!(
            GpuNames::from_hardware_database("AD102 [GeForce RTX 4090]").preferred_name(),
            "GeForce RTX 4090"
        );
        assert_eq!(
            GpuNames::from_hardware_database("Navi 31").preferred_name(),
            "Navi 31"
        );
    }
}
