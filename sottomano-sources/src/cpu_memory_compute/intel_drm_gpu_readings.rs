use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use uom::si::{
    f64::{Frequency, Ratio},
    frequency::megahertz,
    ratio::ratio,
};

use super::{
    cpu_power_draw::EnergyCounter,
    memory_usage::read_meminfo,
    pci_gpu_readings::{
        DiscoveredGpu, GpuClocks, GpuHandle, GpuMemory, GpuModel, GpuProcess, GpuProcesses,
        GpuSnapshot, GpuTemperatureLimits, GpuThrottleReason, GpuVendor, PciBusId, SharedGpuMemory,
    },
};
use crate::{
    blocking_background_work::{blocking, blocking_poll},
    host_machine_identity::udev_hardware_identity::Adapter,
    system_protocol_clients::sysfs_attribute_reads::{
        Error, list_dir, parse_field, read_i64, read_trimmed,
    },
};

const XE_PCI_DRIVER: &str = "/sys/bus/pci/drivers/xe";
const KERNEL_RELEASE: &str = "/proc/sys/kernel/osrelease";
const PROC: &str = "/proc";
const UNCORE_RAPL_ZONE: &str = "uncore";
const TTM_PAGES_LIMIT: &str = "/sys/module/ttm/parameters/pages_limit";
const SAMPLE_GAP: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntelDrmCard {
    pub bus: PciBusId,
    device: PathBuf,
    uncore_energy_zone: Option<PathBuf>,
    swap_out_threshold: Option<ByteSize>,
}

impl IntelDrmCard {
    fn render_gt(&self) -> PathBuf {
        self.device.join("tile0/gt0")
    }

    fn render_frequency(&self) -> PathBuf {
        self.render_gt().join("freq0")
    }

    fn render_idle_milliseconds(&self) -> Result<i64, Error> {
        parse_field(&self.render_gt().join("gtidle/idle_residency_ms"))
    }

    fn uncore_energy(&self) -> Option<EnergyCounter> {
        EnergyCounter::read(self.uncore_energy_zone.as_deref()?)
    }
}

pub(super) async fn discover_xe_gpus() -> Vec<DiscoveredGpu> {
    blocking("intel gpu discovery", || {
        list_dir(XE_PCI_DRIVER)
            .into_iter()
            .filter(|entry| entry.join("vendor").exists())
            .filter_map(|entry| discovered_xe_gpu(&entry))
            .collect()
    })
    .await
}

fn discovered_xe_gpu(driver_entry: &Path) -> Option<DiscoveredGpu> {
    let bus = PciBusId(driver_entry.file_name()?.to_str()?.to_owned());
    let device = fs::canonicalize(driver_entry).ok()?;
    let card = IntelDrmCard {
        bus: bus.clone(),
        uncore_energy_zone: sits_on_root_bus(&device).then(uncore_energy_zone).flatten(),
        swap_out_threshold: ttm_swap_out_threshold(),
        device,
    };
    Some(DiscoveredGpu {
        model: Some(xe_model(&card.device)),
        handle: Ok(GpuHandle::IntelDrm(card)),
        bus,
    })
}

fn sits_on_root_bus(device: &Path) -> bool {
    device
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("pci"))
}

fn ttm_swap_out_threshold() -> Option<ByteSize> {
    let pages = u64::try_from(read_i64(TTM_PAGES_LIMIT)?).ok()?;
    Some(ByteSize::b(pages * procfs::page_size()))
}

fn uncore_energy_zone() -> Option<PathBuf> {
    EnergyCounter::read_all()
        .into_iter()
        .find(|counter| counter.name == UNCORE_RAPL_ZONE)
        .map(|counter| counter.zone)
}

fn xe_model(device: &Path) -> GpuModel {
    GpuModel {
        name: Adapter::of_device(device)
            .and_then(|adapter| adapter.model)
            .unwrap_or_else(|| "Intel graphics".to_owned()),
        driver_version: read_trimmed(KERNEL_RELEASE)
            .map_or_else(|| "xe".to_owned(), |release| format!("xe, Linux {release}")),
        temperature_limits: GpuTemperatureLimits::default(),
        power_limit: None,
    }
}

pub(super) async fn read_snapshot(card: IntelDrmCard) -> Result<GpuSnapshot, Error> {
    blocking_poll(move || {
        let idle_before = card.render_idle_milliseconds()?;
        let energy_before = card.uncore_energy();
        let started = Instant::now();
        thread::sleep(SAMPLE_GAP);
        let idle_after = card.render_idle_milliseconds()?;
        let energy_after = card.uncore_energy();
        let elapsed = started.elapsed();
        let idle_share = (idle_after - idle_before) as f64 / elapsed.as_secs_f64() / 1000.0;
        Ok(GpuSnapshot {
            vendor: GpuVendor::Intel,
            load: Ratio::new::<ratio>((1.0 - idle_share).clamp(0.0, 1.0)),
            graphics_clock: megahertz_field(&card.render_frequency().join("act_freq")).ok(),
            memory: GpuMemory::SharedWithSystem(shared_gpu_memory(&card)),
            temperature: None,
            power_draw: energy_after
                .zip(energy_before)
                .map(|(after, before)| after.draw_since(&before, elapsed)),
        })
    })
    .await
}

pub(super) async fn read_clocks(card: IntelDrmCard) -> Result<GpuClocks, Error> {
    blocking_poll(move || {
        let frequency = card.render_frequency();
        Ok(GpuClocks {
            performance_state: None,
            graphics_clock_ceiling: megahertz_field(&frequency.join("max_freq")).ok(),
            memory_clock: None,
            fan: None,
            throttle_reasons: throttle_reasons(&frequency.join("throttle")),
        })
    })
    .await
}

fn megahertz_field(path: &Path) -> Result<Frequency, Error> {
    parse_field::<f64>(path).map(Frequency::new::<megahertz>)
}

fn throttle_reasons(throttle: &Path) -> Vec<GpuThrottleReason> {
    if read_trimmed(throttle.join("status")).as_deref() != Some("1") {
        return Vec::new();
    }
    [
        ("reason_pl1", GpuThrottleReason::SustainedPowerLimit),
        ("reason_pl2", GpuThrottleReason::BurstPowerLimit),
        ("reason_pl4", GpuThrottleReason::PeakPowerLimit),
        ("reason_prochot", GpuThrottleReason::ProcessorHot),
        ("reason_ratl", GpuThrottleReason::RunningAverageThermalLimit),
        ("reason_thermal", GpuThrottleReason::Thermal),
        ("reason_vr_tdc", GpuThrottleReason::VoltageRegulatorCurrent),
        (
            "reason_vr_thermalert",
            GpuThrottleReason::VoltageRegulatorThermal,
        ),
    ]
    .into_iter()
    .filter(|(file, _)| read_i64(throttle.join(file)) == Some(1))
    .map(|(_, reason)| reason)
    .collect()
}

pub(super) async fn read_processes(card: IntelDrmCard) -> Result<GpuProcesses, Error> {
    blocking_poll(move || {
        Ok(GpuProcesses {
            processes: drm_client_processes(&card.bus),
        })
    })
    .await
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct DrmClientMemory {
    resident: ByteSize,
    shared: ByteSize,
    active: ByteSize,
    purgeable: ByteSize,
}

impl DrmClientMemory {
    fn private(self) -> ByteSize {
        ByteSize::b(self.resident.as_u64().saturating_sub(self.shared.as_u64()))
    }

    fn combined_with(self, other: Self) -> Self {
        DrmClientMemory {
            resident: self.resident + other.resident,
            shared: self.shared + other.shared,
            active: self.active + other.active,
            purgeable: self.purgeable + other.purgeable,
        }
    }
}

struct DrmClient {
    pid: u32,
    client_id: u64,
    memory: DrmClientMemory,
}

fn drm_clients(bus: &PciBusId) -> Vec<DrmClient> {
    let mut seen_clients = HashSet::new();
    list_dir(PROC)
        .iter()
        .filter_map(|process| Some((process, process.file_name()?.to_str()?.parse().ok()?)))
        .flat_map(|(process, pid)| drm_clients_of_process(process, pid, bus))
        .filter(|client| seen_clients.insert(client.client_id))
        .collect()
}

fn shared_gpu_memory(card: &IntelDrmCard) -> SharedGpuMemory {
    let total = drm_clients(&card.bus)
        .iter()
        .fold(DrmClientMemory::default(), |total, client| {
            total.combined_with(client.memory)
        });
    SharedGpuMemory {
        private: total.private(),
        shared: total.shared,
        active: total.active,
        purgeable: total.purgeable,
        system: read_meminfo().ok(),
        swap_out_threshold: card.swap_out_threshold,
    }
}

fn drm_client_processes(bus: &PciBusId) -> Vec<GpuProcess> {
    let mut private_by_pid: BTreeMap<u32, ByteSize> = BTreeMap::new();
    for client in drm_clients(bus) {
        *private_by_pid.entry(client.pid).or_default() += client.memory.private();
    }
    let mut processes: Vec<GpuProcess> = private_by_pid
        .into_iter()
        .map(|(pid, private)| GpuProcess {
            pid,
            name: read_trimmed(Path::new(PROC).join(pid.to_string()).join("comm")),
            resident_memory: Some(private),
        })
        .collect();
    processes.sort_by_key(|process| Reverse(process.resident_memory));
    processes
}

fn drm_clients_of_process(process: &Path, pid: u32, bus: &PciBusId) -> Vec<DrmClient> {
    list_dir(process.join("fd"))
        .into_iter()
        .filter(|fd| fs::read_link(fd).is_ok_and(|target| target.starts_with("/dev/dri")))
        .filter_map(|fd| {
            let fdinfo = fs::read_to_string(process.join("fdinfo").join(fd.file_name()?)).ok()?;
            drm_client_from_fdinfo(&fdinfo, pid, bus)
        })
        .collect()
}

fn drm_client_from_fdinfo(fdinfo: &str, pid: u32, bus: &PciBusId) -> Option<DrmClient> {
    let fields: BTreeMap<&str, &str> = fdinfo
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect();
    (fields.get("drm-pdev") == Some(&bus.0.as_str())).then_some(())?;
    let summed_over_regions = |prefix: &str| -> ByteSize {
        fields
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .filter_map(|(_, value)| fdinfo_memory_amount(value))
            .sum()
    };
    Some(DrmClient {
        pid,
        client_id: fields.get("drm-client-id")?.parse().ok()?,
        memory: DrmClientMemory {
            resident: summed_over_regions("drm-resident-"),
            shared: summed_over_regions("drm-shared-"),
            active: summed_over_regions("drm-active-"),
            purgeable: summed_over_regions("drm-purgeable-"),
        },
    })
}

fn fdinfo_memory_amount(value: &str) -> Option<ByteSize> {
    let (amount, unit) = value.split_once(' ').unwrap_or((value, ""));
    let amount: u64 = amount.parse().ok()?;
    match unit {
        "" => Some(ByteSize::b(amount)),
        "KiB" => Some(ByteSize::kib(amount)),
        "MiB" => Some(ByteSize::mib(amount)),
        "GiB" => Some(ByteSize::gib(amount)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;

    use super::{DrmClientMemory, PciBusId, drm_client_from_fdinfo, fdinfo_memory_amount};

    const XE_FDINFO_LINES: [&str; 14] = [
        "pos:\t0",
        "flags:\t02100002",
        "drm-driver:\txe",
        "drm-client-id:\t46",
        "drm-pdev:\t0000:00:02.0",
        "drm-total-system:\t0",
        "drm-resident-system:\t4 MiB",
        "drm-purgeable-system:\t1 MiB",
        "drm-total-gtt:\t179640 KiB",
        "drm-shared-gtt:\t92 MiB",
        "drm-active-gtt:\t92 MiB",
        "drm-resident-gtt:\t179640 KiB",
        "drm-resident-stolen:\t0",
        "drm-cycles-rcs:\t8667354162",
    ];

    #[test]
    fn sums_each_memory_kind_over_regions_of_matching_device() {
        let fdinfo = XE_FDINFO_LINES.join("\n");
        let client =
            drm_client_from_fdinfo(&fdinfo, 7, &PciBusId("0000:00:02.0".to_owned())).unwrap();
        assert_eq!(client.client_id, 46);
        assert_eq!(
            client.memory,
            DrmClientMemory {
                resident: ByteSize::kib(179_640 + 4 * 1024),
                shared: ByteSize::mib(92),
                active: ByteSize::mib(92),
                purgeable: ByteSize::mib(1),
            }
        );
    }

    #[test]
    fn private_memory_excludes_shared_buffers() {
        let memory = DrmClientMemory {
            resident: ByteSize::mib(100),
            shared: ByteSize::mib(30),
            ..DrmClientMemory::default()
        };
        assert_eq!(memory.private(), ByteSize::mib(70));
    }

    #[test]
    fn ignores_clients_of_other_devices() {
        let fdinfo = XE_FDINFO_LINES.join("\n");
        assert!(drm_client_from_fdinfo(&fdinfo, 7, &PciBusId("0000:03:00.0".to_owned())).is_none());
    }

    #[test]
    fn reads_fdinfo_memory_units() {
        assert_eq!(fdinfo_memory_amount("92 MiB"), Some(ByteSize::mib(92)));
        assert_eq!(fdinfo_memory_amount("0"), Some(ByteSize::b(0)));
        assert_eq!(fdinfo_memory_amount("3 PiB"), None);
    }
}
