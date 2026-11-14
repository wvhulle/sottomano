use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    vmware_machines::{VmwareMachine, VmwareMachines},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::vmware_machines_group::{VmwareMachinesGroup, VmwareMachinesMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_MACHINE_PID: i32 = 63_118;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = VmwareMachinesGroup::default();
    let _ = group.update(
        VmwareMachinesMessage::Detail(SnapshotDelivery::fixture_delivered(
            Ok(VmwareMachines {
                machines: vec![VmwareMachine {
                    pid: HOVERED_MACHINE_PID,
                    machine_name: "Ubuntu 26.04".to_owned(),
                    configuration_file: PathBuf::from("/home/user/vmware/Ubuntu/Ubuntu.vmx"),
                    guest_os: Some("ubuntu-64".to_owned()),
                    guest_memory: Some(ByteSize::gib(4)),
                    guest_cores: Some(2),
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
