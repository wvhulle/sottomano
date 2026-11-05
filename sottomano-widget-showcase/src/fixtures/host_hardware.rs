use bytesize::ByteSize;
use chrono::{TimeZone, Utc};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::cpu_topology::{Chiplet, CoreGroup, CoreRole, CpuTopology, PhysicalCore},
    host_machine_identity::{
        hostname1_os_identity::MachineInfo,
        udev_hardware_identity::{
            CacheInstances, CacheKind, CacheLevel, CachePosition, CpuArchitecture, CpuCoreCounts,
            CpuIdentity, CpuModel, CpuVendor, HostHardware, MemoryModule, MemoryModuleGroup,
            MemoryModules,
        },
    },
};
use uom::si::{f64::Information, information::mebibyte};

const MSI_PCI_VENDOR: u16 = 0x1462;
pub const CORES_PER_CHIPLET: u32 = 6;
pub const PHYSICAL_CORE_COUNT: u32 = 2 * CORES_PER_CHIPLET;
pub const STACKED_CACHE_CHIPLET: usize = 0;
pub const HIGH_CLOCK_CHIPLET: usize = 1;

pub fn logical_cpus() -> impl Iterator<Item = u32> {
    0..2 * PHYSICAL_CORE_COUNT
}

pub fn chiplet_of(logical_cpu: u32) -> usize {
    usize::from(logical_cpu % PHYSICAL_CORE_COUNT >= CORES_PER_CHIPLET)
}

pub fn dual_chiplet_topology() -> CpuTopology {
    CpuTopology::from_core_groups(vec![
        chiplet_core_group(CoreRole::StackedCacheCores, STACKED_CACHE_CHIPLET, 96.0),
        chiplet_core_group(CoreRole::HighClockCores, HIGH_CLOCK_CHIPLET, 32.0),
    ])
}

fn chiplet_core_group(role: CoreRole, position: usize, shared_cache_mebibytes: f64) -> CoreGroup {
    let first_core = u32::try_from(position).expect("two chiplets") * CORES_PER_CHIPLET;
    CoreGroup::new(
        role,
        (first_core..first_core + CORES_PER_CHIPLET)
            .map(|core| PhysicalCore {
                threads: vec![core, core + PHYSICAL_CORE_COUNT],
            })
            .collect(),
        vec![Chiplet {
            position,
            shared_cache: Information::new::<mebibyte>(shared_cache_mebibytes),
        }],
    )
}

pub fn ryzen_workstation_delivery() -> Box<SnapshotDelivery<HostHardware>> {
    Box::new(SnapshotDelivery::single_read(Ok(ryzen_workstation())))
}

fn ryzen_workstation() -> HostHardware {
    HostHardware {
        cpu: CpuModel {
            identity: CpuIdentity {
                vendor: Some(CpuVendor::Amd),
                name: Some("AMD Ryzen 9 7900X3D 12-Core Processor".to_owned()),
                microcode: Some("0xa601206".to_owned()),
            },
            architecture: CpuArchitecture {
                microarchitecture: Some("Zen 4".to_owned()),
                process: Some("TSMC 5nm".to_owned()),
                released: Some("2023".to_owned()),
                x86_64_level: Some("x86-64-v4".to_owned()),
            },
            topology: CpuCoreCounts {
                cores: Some(12),
                threads: Some(24),
            },
            caches: vec![CacheLevel {
                position: CachePosition {
                    level: 3,
                    kind: CacheKind::Unified,
                },
                instances: vec![
                    CacheInstances {
                        size: ByteSize::mib(96),
                        count: 1,
                    },
                    CacheInstances {
                        size: ByteSize::mib(32),
                        count: 1,
                    },
                ],
            }],
        },
        memory: MemoryModules(vec![MemoryModuleGroup {
            module: MemoryModule {
                manufacturer: Some("Kingston".to_owned()),
                part_number: Some("KF556C36-32".to_owned()),
                kind: Some("DDR5".to_owned()),
                speed_mts: Some(5600),
                size: ByteSize::gib(32),
            },
            count: 2,
        }]),
        machine: MachineInfo {
            board_pci_vendor: Some(MSI_PCI_VENDOR),
            hardware_vendor: Some("Micro-Star International Co., Ltd.".to_owned()),
            hardware_model: Some("MAG B650 TOMAHAWK WIFI (MS-7D75)".to_owned()),
            firmware_vendor: Some("American Megatrends International, LLC.".to_owned()),
            firmware_version: Some("1.D0".to_owned()),
            firmware_date: Utc.with_ymd_and_hms(2025, 11, 4, 0, 0, 0).single(),
        },
    }
}
