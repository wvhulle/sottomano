use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cgroup_memory_limits::{LimitedCgroup, LimitedCgroups},
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    system_protocol_clients::sysfs_cgroup_memory::{
        CgroupMemoryReading, MemoryBound, MemoryEventCounts,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::cgroup_memory_limits_group::{
        CgroupMemoryLimitsGroup, CgroupMemoryLimitsMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_CGROUP_PATH: &str = "/ci.slice/ci-runners.slice";

fn ci_runners_near_their_throttle_mark() -> LimitedCgroup {
    LimitedCgroup {
        unit_name: "ci-runners.slice".to_owned(),
        cgroup_path: HOVERED_CGROUP_PATH.to_owned(),
        memory: CgroupMemoryReading {
            current: ByteSize::mib(9_830),
            high: MemoryBound::Bytes(ByteSize::gib(10)),
            max: MemoryBound::Bytes(ByteSize::gib(12)),
            swap_current: Some(ByteSize::mib(1_240)),
            swap_max: MemoryBound::Bytes(ByteSize::gib(4)),
            stalled_percent_last_ten_seconds: Some(4.2),
            event_counts: MemoryEventCounts {
                throttled_at_high: 2,
                reached_max: 0,
                oom_kills: 0,
            },
        },
    }
}

fn nix_daemon_building() -> LimitedCgroup {
    LimitedCgroup {
        unit_name: "nix-daemon.service".to_owned(),
        cgroup_path: "/system.slice/nix-daemon.service".to_owned(),
        memory: CgroupMemoryReading {
            current: ByteSize::mib(3_460),
            high: MemoryBound::Bytes(ByteSize::gib(14)),
            max: MemoryBound::Unbounded,
            swap_current: Some(ByteSize::mib(80)),
            swap_max: MemoryBound::Unbounded,
            stalled_percent_last_ten_seconds: Some(0.0),
            event_counts: MemoryEventCounts::default(),
        },
    }
}

fn guest_machines() -> LimitedCgroup {
    LimitedCgroup {
        unit_name: "machine.slice".to_owned(),
        cgroup_path: "/machine.slice".to_owned(),
        memory: CgroupMemoryReading {
            current: ByteSize::mib(940),
            high: MemoryBound::Unbounded,
            max: MemoryBound::Unbounded,
            swap_current: Some(ByteSize::mib(310)),
            swap_max: MemoryBound::Unbounded,
            stalled_percent_last_ten_seconds: Some(0.0),
            event_counts: MemoryEventCounts::default(),
        },
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = CgroupMemoryLimitsGroup::default();
    let _ = group.update(
        CgroupMemoryLimitsMessage::Cgroups(SnapshotDelivery::fixture_delivered(
            Ok(LimitedCgroups {
                cgroups: vec![
                    ci_runners_near_their_throttle_mark(),
                    nix_daemon_building(),
                    guest_machines(),
                ],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::CgroupMemoryLimits(Box::new(group))
}
