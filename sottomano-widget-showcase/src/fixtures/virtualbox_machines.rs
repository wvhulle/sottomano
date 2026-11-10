use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    virtualbox_machines::{VirtualBoxFrontend, VirtualBoxMachine, VirtualBoxMachines},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::virtualbox_machines_group::{
        VirtualBoxMachinesGroup, VirtualBoxMachinesMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_MACHINE_PID: i32 = 61_402;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = VirtualBoxMachinesGroup::default();
    let _ = group.update(
        VirtualBoxMachinesMessage::Machines(SnapshotDelivery::fixture_delivered(
            Ok(VirtualBoxMachines {
                machines: vec![VirtualBoxMachine {
                    pid: HOVERED_MACHINE_PID,
                    machine_name: "windows11".to_owned(),
                    machine_reference: "6c1f1d2e-4b7a-4e8f-9a3b-0d5c2e7f8a91".to_owned(),
                    frontend: VirtualBoxFrontend::Headless,
                    guest_memory: Some(ByteSize::gib(8)),
                    guest_cores: Some(4),
                    settings_file: Some(PathBuf::from(
                        "/home/user/VirtualBox VMs/windows11/windows11.vbox",
                    )),
                    started_at: clock.wall - Duration::from_mins(48),
                    memory_in_use: ByteSize::mib(8_420),
                }],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::VirtualBoxMachines(Box::new(group))
}
