use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    apptainer_containers::{ApptainerContainer, ApptainerContainers},
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::apptainer_containers_group::{
        ApptainerContainersGroup, ApptainerContainersMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_RUNTIME_PID: i32 = 70_215;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = ApptainerContainersGroup::default();
    group.update(
        ApptainerContainersMessage::Containers(SnapshotDelivery::fixture_delivered(
            Ok(ApptainerContainers {
                containers: vec![ApptainerContainer {
                    runtime_pid: HOVERED_RUNTIME_PID,
                    runtime_title: "Apptainer runtime parent".to_owned(),
                    image_file: Some(PathBuf::from("/home/user/images/pytorch_24.08.sif")),
                    running_command: Some("python3 train.py --epochs 40".to_owned()),
                    owner_uid: 1000,
                    owner_name: Some("user".to_owned()),
                    started_at: clock.wall - Duration::from_mins(73),
                    memory_in_use: ByteSize::mib(5_870),
                }],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::ApptainerContainers(Box::new(group))
}
