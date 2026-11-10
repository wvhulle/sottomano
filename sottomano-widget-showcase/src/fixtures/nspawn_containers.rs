use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
    time::Duration,
};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    nspawn_containers::machined_nspawn_containers::{
        ContainerRegistration, NspawnContainer, NspawnContainersReadout, RegisteredContainerMachine,
    },
    systemd_units_journal::unit_active_state::{UnitActiveState, UnitActivity},
};
use sottomano_widgets::{
    clock::Clock,
    group_items::unit_switch::UnitSwitchMessage,
    widget_groups::nspawn_containers_group::{NspawnContainersGroup, NspawnContainersMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_CONTAINER_NAME: &str = "arr-proton";

fn running_media_container(clock: Clock) -> NspawnContainer {
    NspawnContainer {
        machine_name: HOVERED_CONTAINER_NAME.to_owned(),
        unit_name: "container@arr-proton.service".to_owned(),
        activity: UnitActivity::Running,
        registration: ContainerRegistration::Registered(RegisteredContainerMachine {
            leader_pid: 31_104,
            started_at: Some(clock.wall - Duration::from_hours(26)),
            ip_addresses: vec![
                IpAddr::V4(Ipv4Addr::new(10, 200, 0, 2)),
                IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 2)),
            ],
            root_directory: PathBuf::from("/var/lib/nixos-containers/arr-proton"),
            memory_in_use: Some(ByteSize::mib(612)),
        }),
    }
}

fn running_build_container(clock: Clock) -> NspawnContainer {
    NspawnContainer {
        machine_name: "debian-build".to_owned(),
        unit_name: "systemd-nspawn@debian-build.service".to_owned(),
        activity: UnitActivity::Running,
        registration: ContainerRegistration::Registered(RegisteredContainerMachine {
            leader_pid: 48_211,
            started_at: Some(clock.wall - Duration::from_mins(7)),
            ip_addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 77, 14))],
            root_directory: PathBuf::from("/var/lib/machines/debian-build"),
            memory_in_use: Some(ByteSize::mib(1_840)),
        }),
    }
}

fn stopped_backup_container() -> NspawnContainer {
    NspawnContainer {
        machine_name: "backup-restore".to_owned(),
        unit_name: "container@backup-restore.service".to_owned(),
        activity: UnitActivity::Stopped,
        registration: ContainerRegistration::Unregistered,
    }
}

fn switch_activity(machine_name: &str, activity: UnitActivity) -> NspawnContainersMessage {
    NspawnContainersMessage::Switch(
        machine_name.to_owned(),
        UnitSwitchMessage::ActiveState(SnapshotDelivery::live_push(Ok(UnitActiveState {
            activity,
        }))),
    )
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = NspawnContainersGroup::default();
    let containers = vec![
        running_media_container(clock),
        stopped_backup_container(),
        running_build_container(clock),
    ];
    let switch_messages: Vec<NspawnContainersMessage> = containers
        .iter()
        .map(|container| switch_activity(&container.machine_name, container.activity))
        .collect();
    let _ = group.update(
        NspawnContainersMessage::Containers(SnapshotDelivery::fixture_delivered(
            Ok(NspawnContainersReadout { containers }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    for switch_message in switch_messages {
        let _ = group.update(switch_message, clock);
    }
    ShowcasedGroup::NspawnContainers(Box::new(group))
}
