use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    flatpak_sandboxes::{FlatpakSandbox, FlatpakSandboxes},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::flatpak_sandboxes_group::{FlatpakSandboxesGroup, FlatpakSandboxesMessage},
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
        instance_id: instance_id.to_owned(),
        application_id: application_id.to_owned(),
        runtime_reference: Some("runtime/org.freedesktop.Platform/x86_64/24.08".to_owned()),
        branch: Some("stable".to_owned()),
        sandbox_pid,
        started_at: Some(clock.wall - running_for),
        memory_in_use: Some(memory_in_use),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = FlatpakSandboxesGroup::default();
    group.update(
        FlatpakSandboxesMessage::Sandboxes(SnapshotDelivery::fixture_delivered(
            Ok(FlatpakSandboxes {
                sandboxes: vec![
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
