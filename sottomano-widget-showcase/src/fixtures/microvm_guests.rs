use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    microvm_guests::{
        MicrovmGuest, MicrovmGuests,
        hypervisor_command_lines::{HypervisorLaunch, MicrovmHypervisor},
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::microvm_guests_group::{MicrovmGuestsGroup, MicrovmGuestsMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_MICROVM_PID: i32 = 48_213;

fn ci_builder_microvm(clock: Clock) -> MicrovmGuest {
    MicrovmGuest {
        pid: HOVERED_MICROVM_PID,
        machine_name: "ci-builder".to_owned(),
        hypervisor: MicrovmHypervisor::CloudHypervisor,
        unit_name: Some("microvm@ci-builder.service".to_owned()),
        launch: HypervisorLaunch {
            named_machine: None,
            guest_memory: Some(ByteSize::gib(4)),
            guest_cores: Some(4),
            kernel: Some(PathBuf::from("/nix/store/9w2k1v3x-linux-6.18.4/bzImage")),
            api_socket: Some(PathBuf::from(
                "/var/lib/microvms/ci-builder/ci-builder.sock",
            )),
            config_file: None,
        },
        started_at: clock.wall - Duration::from_mins(95),
        memory_in_use: ByteSize::mib(2_630),
    }
}

fn sandbox_firecracker(clock: Clock) -> MicrovmGuest {
    MicrovmGuest {
        pid: 51_007,
        machine_name: "sandbox".to_owned(),
        hypervisor: MicrovmHypervisor::Firecracker,
        unit_name: None,
        launch: HypervisorLaunch {
            named_machine: Some("sandbox".to_owned()),
            guest_memory: Some(ByteSize::mib(256)),
            guest_cores: Some(1),
            kernel: Some(PathBuf::from("/srv/firecracker/vmlinux")),
            api_socket: Some(PathBuf::from("/run/user/1000/firecracker/sandbox.sock")),
            config_file: Some(PathBuf::from("/srv/firecracker/sandbox.json")),
        },
        started_at: clock.wall - Duration::from_mins(7),
        memory_in_use: ByteSize::mib(141),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = MicrovmGuestsGroup::default();
    let _ = group.update(
        MicrovmGuestsMessage::Machines(SnapshotDelivery::fixture_delivered(
            Ok(MicrovmGuests {
                machines: vec![ci_builder_microvm(clock), sandbox_firecracker(clock)],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::MicrovmGuests(Box::new(group))
}
