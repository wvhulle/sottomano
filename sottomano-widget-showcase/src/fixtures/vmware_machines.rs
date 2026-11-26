use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_family_snapshots::FamilyDetails,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    vmware_machines::{VmwareGuestOs, VmwareMachine, VmwareMachineListing},
};
use sottomano_widgets::{
    clock::Clock,
    panel_menus::virtualization_menu::vmware_machines_group::{
        VmwareMachinesGroup, VmwareMachinesMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_MACHINE_PID: i32 = 63_118;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = VmwareMachinesGroup::default();
    let _ = group.update(
        VmwareMachinesMessage::Detail(SnapshotDelivery::fixture_delivered(
            Ok(FamilyDetails {
                details: vec![VmwareMachine {
                    listing: VmwareMachineListing {
                        pid: HOVERED_MACHINE_PID,
                        machine_name: "Ubuntu 26.04".to_owned(),
                        configuration_file: PathBuf::from("/home/user/vmware/Ubuntu/Ubuntu.vmx"),
                    },
                    guest_os: VmwareGuestOs::Declared("ubuntu-64".to_owned()),
                    guest_memory: GuestMemorySize::Configured(ByteSize::gib(4)),
                    guest_cores: GuestCoreCount::Configured(2),
                    started_at: clock.wall - Duration::from_mins(22),
                    memory_in_use: ByteSize::mib(4_310),
                }],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::VmwareMachines(Box::new(group))
}
