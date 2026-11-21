use bytesize::ByteSize;
use chrono::{TimeZone, Utc};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::cpu_topology::{Chiplet, CoreGroup, CoreRole, CpuTopology, PhysicalCore},
    host_machine_identity::{
        fwupd_firmware_upgrades::FirmwareReleaseDate,
        hostname1_os_identity::MachineInfo,
        udev_hardware_identity::{
            CacheInstances, CacheKind, CacheLevel, CachePosition, CpuArchitecture, CpuCoreCounts,
            CpuIdentity, CpuModel, CpuVendor, HardwareText, HostHardware, MemoryModule,
            MemorySlotModule, MemoryTransferRate,
        },
    },
};
use uom::si::{f64::Information, information::mebibyte};

pub const CORES_PER_CHIPLET: u32 = 6;
pub const PHYSICAL_CORE_COUNT: u32 = 2 * CORES_PER_CHIPLET;
pub const STACKED_CACHE_CHIPLET: usize = 0;
pub const HIGH_CLOCK_CHIPLET: usize = 1;

pub fn logical_cpus() -> impl Iterator<Item = u32> {
    0..2 * PHYSICAL_CORE_COUNT
}

pub fn owning_chiplet(logical_cpu: u32) -> usize {
    usize::from(logical_cpu % PHYSICAL_CORE_COUNT >= CORES_PER_CHIPLET)
}

pub fn dual_chiplet_topology() -> CpuTopology {
    CpuTopology::from(vec![
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
                vendor: CpuVendor::Amd,
                name: HardwareText::Reported("AMD Ryzen 9 7900X3D 12-Core Processor".to_owned()),
                microcode: HardwareText::Reported("0xa601206".to_owned()),
            },
            architecture: CpuArchitecture {
                microarchitecture: HardwareText::Reported("Zen 4".to_owned()),
                process: HardwareText::Reported("TSMC 5nm".to_owned()),
                released: HardwareText::Reported("2023".to_owned()),
                x86_64_level: HardwareText::Reported("x86-64-v4".to_owned()),
            },
            topology: CpuCoreCounts::Counted {
                cores: 12,
                threads: 24,
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
        memory: vec![
            MemorySlotModule {
                locator: "DIMM_A1".to_owned(),
                module: MemoryModule {
                    manufacturer: HardwareText::Reported("Kingston".to_owned()),
                    part_number: HardwareText::Reported("KF556C36-32".to_owned()),
                    kind: HardwareText::Reported("DDR5".to_owned()),
                    transfer_rate: MemoryTransferRate::Reported {
                        megatransfers_per_second: 5600,
                    },
                    size: ByteSize::gib(32),
                },
            },
            MemorySlotModule {
                locator: "DIMM_B1".to_owned(),
                module: MemoryModule {
                    manufacturer: HardwareText::Reported("Kingston".to_owned()),
                    part_number: HardwareText::Reported("KF556C36-32".to_owned()),
                    kind: HardwareText::Reported("DDR5".to_owned()),
                    transfer_rate: MemoryTransferRate::Reported {
                        megatransfers_per_second: 5600,
                    },
                    size: ByteSize::gib(32),
                },
            },
        ],
        machine: MachineInfo {
            hardware_vendor: HardwareText::Reported(
                "Micro-Star International Co., Ltd.".to_owned(),
            ),
            hardware_model: HardwareText::Reported("MAG B650 TOMAHAWK WIFI (MS-7D75)".to_owned()),
            firmware_vendor: HardwareText::Reported(
                "American Megatrends International, LLC.".to_owned(),
            ),
            firmware_version: HardwareText::Reported("1.D0".to_owned()),
            firmware_date: FirmwareReleaseDate::Dated(
                Utc.with_ymd_and_hms(2025, 11, 4, 0, 0, 0)
                    .single()
                    .expect("a valid calendar date"),
            ),
        },
    }
}
