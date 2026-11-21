use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_family_snapshots::FamilyDetails,
    container_run_state::ContainerRunState,
    container_runtime_facts::ContainerMemoryUse,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    lxc_containers::{LxcConfigFile, LxcContainer, LxcContainerListing},
};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::lxc_containers_group::{LxcContainersGroup, LxcContainersMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_CONTAINER_NAME: &str = "media";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = LxcContainersGroup::default();
    let _ = group.update(
        LxcContainersMessage::Detail(SnapshotDelivery::fixture_delivered(
            Ok(FamilyDetails {
                details: vec![
                    LxcContainer {
                        listing: LxcContainerListing {
                            container_name: "backup".to_owned(),
                            run_state: ContainerRunState::Stopped,
                        },
                        memory_in_use: ContainerMemoryUse::Unaccounted,
                        configured_addresses: Vec::new(),
                        config_path: LxcConfigFile::InLxcPath(PathBuf::from(
                            "/var/lib/lxc/backup/config",
                        )),
                    },
                    LxcContainer {
                        listing: LxcContainerListing {
                            container_name: HOVERED_CONTAINER_NAME.to_owned(),
                            run_state: ContainerRunState::Running,
                        },
                        memory_in_use: ContainerMemoryUse::Measured(ByteSize::mib(530)),
                        configured_addresses: vec!["10.0.3.15/24".to_owned()],
                        config_path: LxcConfigFile::InLxcPath(PathBuf::from(
                            "/var/lib/lxc/media/config",
                        )),
                    },
                ],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::LxcContainers(Box::new(group))
}
