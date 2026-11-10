use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    lxc_containers::{LxcContainer, LxcContainers},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::lxc_containers_group::{LxcContainersGroup, LxcContainersMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_CONTAINER_NAME: &str = "media";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = LxcContainersGroup::default();
    group.update(
        LxcContainersMessage::Containers(SnapshotDelivery::fixture_delivered(
            Ok(LxcContainers {
                containers: vec![
                    LxcContainer {
                        container_name: "backup".to_owned(),
                        running: false,
                        memory_in_use: None,
                        configured_addresses: Vec::new(),
                        config_path: Some(PathBuf::from("/var/lib/lxc/backup/config")),
                    },
                    LxcContainer {
                        container_name: HOVERED_CONTAINER_NAME.to_owned(),
                        running: true,
                        memory_in_use: Some(ByteSize::mib(530)),
                        configured_addresses: vec!["10.0.3.15/24".to_owned()],
                        config_path: Some(PathBuf::from("/var/lib/lxc/media/config")),
                    },
                ],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::LxcContainers(Box::new(group))
}
