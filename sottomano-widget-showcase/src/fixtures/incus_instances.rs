use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    incus_instances::{DaemonInstance, DaemonInstancesReadout, InstanceDaemon, InstanceKind},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::incus_instances_group::{IncusInstancesGroup, IncusInstancesMessage},
};

use crate::showcased_group::ShowcasedGroup;

pub const HOVERED_INSTANCE_NAME: &str = "build-box";

fn daemon_instance(
    instance_name: &str,
    kind: InstanceKind,
    running: bool,
    memory_in_use: Option<ByteSize>,
    global_addresses: &[&str],
    image_description: &str,
) -> DaemonInstance {
    DaemonInstance {
        instance_name: instance_name.to_owned(),
        project_name: "default".to_owned(),
        kind,
        running,
        status_text: if running { "Running" } else { "Stopped" }.to_owned(),
        memory_in_use,
        global_addresses: global_addresses
            .iter()
            .map(|global_address| (*global_address).to_owned())
            .collect(),
        image_description: Some(image_description.to_owned()),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = IncusInstancesGroup::default();
    let _ = group.update(
        IncusInstancesMessage::Instances(
            InstanceDaemon::Incus,
            SnapshotDelivery::fixture_delivered(
                Ok(DaemonInstancesReadout::Instances(vec![
                    daemon_instance(
                        HOVERED_INSTANCE_NAME,
                        InstanceKind::Container,
                        true,
                        Some(ByteSize::mib(268)),
                        &["10.41.7.23"],
                        "Debian trixie amd64",
                    ),
                    daemon_instance(
                        "fedora-desktop",
                        InstanceKind::VirtualMachine,
                        false,
                        None,
                        &[],
                        "Fedora 43 amd64",
                    ),
                ])),
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
            ),
        ),
        clock,
    );
    ShowcasedGroup::IncusInstances(Box::new(group))
}
