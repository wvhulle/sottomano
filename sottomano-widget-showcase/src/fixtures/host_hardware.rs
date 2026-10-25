use bytesize::ByteSize;
use chrono::{TimeZone, Utc};
use sottomano_sources::{
    SourceDelivery,
    host_machine_identity::{
        hostname1_os_identity::MachineInfo,
        udev_hardware_identity::{
            CacheInstances, CacheKind, CacheLevel, CachePosition, CpuArchitecture, CpuCoreCounts,
            CpuIdentity, CpuModel, CpuVendor, HostHardware, MemoryModule, MemoryModuleGroup,
            MemoryModules,
        },
    },
};

const ASUS_PCI_VENDOR: u16 = 0x1043;

pub fn ryzen_workstation_delivery() -> Box<SourceDelivery<HostHardware>> {
    Box::new(SourceDelivery::Reading(Ok(ryzen_workstation())))
}

fn ryzen_workstation() -> HostHardware {
    HostHardware {
        cpu: CpuModel {
            identity: CpuIdentity {
                vendor: Some(CpuVendor::Amd),
                name: Some("AMD Ryzen 9 7950X3D 16-Core Processor".to_owned()),
                microcode: Some("0xa601209".to_owned()),
            },
            architecture: CpuArchitecture {
                microarchitecture: Some("Zen 4".to_owned()),
                process: Some("TSMC 5nm".to_owned()),
                released: Some("2023".to_owned()),
                x86_64_level: Some("x86-64-v4".to_owned()),
            },
            topology: CpuCoreCounts {
                cores: Some(16),
                threads: Some(32),
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
                manufacturer: Some("G.Skill".to_owned()),
                part_number: Some("F5-6000J3038F16G".to_owned()),
                kind: Some("DDR5".to_owned()),
                speed_mts: Some(6000),
                size: ByteSize::gib(32),
            },
            count: 2,
        }]),
        machine: MachineInfo {
            board_pci_vendor: Some(ASUS_PCI_VENDOR),
            hardware_vendor: Some("ASUSTeK COMPUTER INC.".to_owned()),
            hardware_model: Some("ROG STRIX X670E-E GAMING WIFI".to_owned()),
            firmware_vendor: Some("American Megatrends Inc.".to_owned()),
            firmware_version: Some("3205".to_owned()),
            firmware_date: Utc.with_ymd_and_hms(2026, 3, 12, 0, 0, 0).single(),
        },
    }
}
