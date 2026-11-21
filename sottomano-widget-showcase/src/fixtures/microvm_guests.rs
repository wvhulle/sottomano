use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_family_snapshots::FamilyDetails,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    microvm_guests::{
        EnclosingUnit, MicrovmGuest, MicrovmGuestListing,
        hypervisor_command_lines::{
            GuestKernelImage, HypervisorApiSocket, HypervisorLaunch, HypervisorMachineName,
            LaunchConfigFile, MicrovmHypervisor,
        },
    },
};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::microvm_guests_group::{MicrovmGuestsGroup, MicrovmGuestsMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_MICROVM_PID: i32 = 48_213;

fn ci_builder_microvm(clock: Clock) -> MicrovmGuest {
    MicrovmGuest {
        listing: MicrovmGuestListing {
            pid: HOVERED_MICROVM_PID,
            machine_name: "ci-builder".to_owned(),
        },
        hypervisor: MicrovmHypervisor::CloudHypervisor,
        enclosing_unit: EnclosingUnit::Service("microvm@ci-builder.service".to_owned()),
        launch: HypervisorLaunch {
            named_machine: HypervisorMachineName::Unnamed,
            guest_memory: GuestMemorySize::Configured(ByteSize::gib(4)),
            guest_cores: GuestCoreCount::Configured(4),
            kernel: GuestKernelImage::Booted(PathBuf::from(
                "/nix/store/9w2k1v3x-linux-6.18.4/bzImage",
            )),
            api_socket: HypervisorApiSocket::Listening(PathBuf::from(
                "/var/lib/microvms/ci-builder/ci-builder.sock",
            )),
            config_file: LaunchConfigFile::CommandLineOnly,
        },
        started_at: clock.wall - Duration::from_mins(95),
        memory_in_use: ByteSize::mib(2_630),
    }
}

fn sandbox_firecracker(clock: Clock) -> MicrovmGuest {
    MicrovmGuest {
        listing: MicrovmGuestListing {
            pid: 51_007,
            machine_name: "sandbox".to_owned(),
        },
        hypervisor: MicrovmHypervisor::Firecracker,
        enclosing_unit: EnclosingUnit::Unscoped,
        launch: HypervisorLaunch {
            named_machine: HypervisorMachineName::Named("sandbox".to_owned()),
            guest_memory: GuestMemorySize::Configured(ByteSize::mib(256)),
            guest_cores: GuestCoreCount::Configured(1),
            kernel: GuestKernelImage::Booted(PathBuf::from("/srv/firecracker/vmlinux")),
            api_socket: HypervisorApiSocket::Listening(PathBuf::from(
                "/run/user/1000/firecracker/sandbox.sock",
            )),
            config_file: LaunchConfigFile::Given(PathBuf::from("/srv/firecracker/sandbox.json")),
        },
        started_at: clock.wall - Duration::from_mins(7),
        memory_in_use: ByteSize::mib(141),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = MicrovmGuestsGroup::default();
    let _ = group.update(
        MicrovmGuestsMessage::Machines(SnapshotDelivery::fixture_delivered(
            Ok(FamilyDetails {
                details: vec![ci_builder_microvm(clock), sandbox_firecracker(clock)],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::MicrovmGuests(Box::new(group))
}
