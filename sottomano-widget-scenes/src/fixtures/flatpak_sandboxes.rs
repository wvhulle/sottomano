use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_family_snapshots::FamilyDetails,
    container_runtime_facts::{ContainerMemoryUse, ContainerStartTime},
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    flatpak_sandboxes::{FlatpakBranch, FlatpakRuntime, FlatpakSandbox, FlatpakSandboxListing},
};
use sottomano_widgets::{
    clock::Clock,
    panel_menus::virtualization_menu::flatpak_sandboxes_group::{
        FlatpakSandboxesGroup, FlatpakSandboxesMessage,
    },
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_INSTANCE_ID: &str = "1849302754";

fn flatpak_sandbox(
    instance_id: &str,
    application_id: &str,
    sandbox_pid: u32,
    memory_in_use: ByteSize,
    running_for: Duration,
    clock: Clock,
) -> FlatpakSandbox {
    FlatpakSandbox {
        listing: FlatpakSandboxListing {
            application_id: application_id.to_owned(),
            instance_id: instance_id.to_owned(),
        },
        runtime_reference: FlatpakRuntime::Reference(
            "runtime/org.freedesktop.Platform/x86_64/24.08".to_owned(),
        ),
        branch: FlatpakBranch::Named("stable".to_owned()),
        sandbox_pid,
        started_at: ContainerStartTime::StartedAt(clock.wall - running_for),
        memory_in_use: ContainerMemoryUse::Measured(memory_in_use),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = FlatpakSandboxesGroup::default();
    let _ = group.update(
        FlatpakSandboxesMessage::Detail(SnapshotDelivery::fixture_delivered(
            Ok(FamilyDetails {
                details: vec![
                    flatpak_sandbox(
                        HOVERED_INSTANCE_ID,
                        "com.spotify.Client",
                        72_310,
                        ByteSize::mib(712),
                        Duration::from_hours(3),
                        clock,
                    ),
                    flatpak_sandbox(
                        "2210043981",
                        "org.gnome.Calculator",
                        80_455,
                        ByteSize::mib(96),
                        Duration::from_mins(12),
                        clock,
                    ),
                ],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::FlatpakSandboxes(Box::new(group))
}
