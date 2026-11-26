use std::{
    net::{Ipv4Addr, Ipv6Addr},
    path::PathBuf,
    time::{Duration, SystemTime},
};

use sottomano_desktop_sources::nspawn_containers::machined_nspawn_containers::{
    ContainerRegistration, RegisteredContainerMachine,
};

use super::*;

fn clock_at_wall(wall: SystemTime) -> Clock {
    Clock {
        wall,
        ..Clock::real()
    }
}

fn running_container(started_at: SystemTime) -> NspawnContainer {
    NspawnContainer {
        machine_name: "arr-proton".to_owned(),
        unit_name: "container@arr-proton.service".to_owned(),
        activity: UnitActivity::Running,
        registration: ContainerRegistration::Registered(RegisteredContainerMachine {
            leader_pid: 3_110_493,
            started_at: ContainerStartTime::StartedAt(started_at),
            ip_addresses: vec![
                IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 2)),
                IpAddr::V4(Ipv4Addr::new(10, 200, 0, 2)),
            ],
            root_directory: PathBuf::from("/var/lib/nixos-containers/arr-proton"),
            memory_in_use: ContainerMemoryUse::Unaccounted,
        }),
    }
}

#[test]
fn a_running_container_is_labelled_with_its_ipv4_address() {
    let container = running_container(SystemTime::UNIX_EPOCH);
    let row = NspawnContainerIndicator {
        container: &container,
        shown_activity: container.activity,
    };
    assert_eq!(row.label(), Some("10.200.0.2".to_owned()));
}

#[test]
fn a_pending_switch_overrides_the_listed_activity() {
    let container = running_container(SystemTime::UNIX_EPOCH);
    let row = NspawnContainerIndicator {
        container: &container,
        shown_activity: UnitActivity::Stopping,
    };
    assert_eq!(row.label(), Some("stopping".to_owned()));
    assert_eq!(row.state(), IndicatorState::Pending);
}

#[test]
fn a_stopped_container_lists_only_its_state_and_unit() {
    let clock = clock_at_wall(SystemTime::UNIX_EPOCH);
    let container = NspawnContainer {
        machine_name: "backup".to_owned(),
        unit_name: "container@backup.service".to_owned(),
        activity: UnitActivity::Stopped,
        registration: ContainerRegistration::Unregistered,
    };
    let row = NspawnContainerIndicator {
        container: &container,
        shown_activity: container.activity,
    };
    assert_eq!(row.detail_entries(clock).len(), 2);
    assert_eq!(row.state(), IndicatorState::Off);
}

#[test]
fn uptime_is_listed_from_the_machine_start_time() {
    let started_at = SystemTime::UNIX_EPOCH;
    let clock = clock_at_wall(started_at + Duration::from_mins(90));
    let container = running_container(started_at);
    let row = NspawnContainerIndicator {
        container: &container,
        shown_activity: container.activity,
    };
    assert!(row.detail_entries(clock).contains(&LabeledValue::new(
        "Uptime",
        ValueCell::duration(Duration::from_mins(90))
    )));
}
