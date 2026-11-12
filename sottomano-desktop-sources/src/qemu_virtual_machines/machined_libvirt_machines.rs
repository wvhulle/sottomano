use std::collections::HashSet;

use zbus::zvariant::OwnedObjectPath;

use crate::systemd_units_journal::machined_machine_registry::{
    MachineClass, RegisteredMachineListing, registered_machine_at, registered_machine_listings,
};

const LIBVIRT_SERVICE_PREFIX: &str = "libvirt";

fn libvirt_virtual_machine_paths(listings: Vec<RegisteredMachineListing>) -> Vec<OwnedObjectPath> {
    listings
        .into_iter()
        .filter(|listing| {
            listing.class() == MachineClass::VirtualMachine
                && listing
                    .registering_service
                    .starts_with(LIBVIRT_SERVICE_PREFIX)
        })
        .map(|listing| listing.machine_path)
        .collect()
}

async fn leader_pid_of(
    system_bus: &zbus::Connection,
    machine_path: OwnedObjectPath,
) -> Option<i32> {
    let machine = registered_machine_at(system_bus, machine_path.clone())
        .await
        .inspect_err(|machine_failure| {
            tracing::debug!(
                "machined machine {}: {machine_failure}",
                machine_path.as_str()
            );
        })
        .ok()?;
    let leader_pid = machine
        .leader()
        .await
        .inspect_err(|leader_failure| {
            tracing::debug!(
                "machined leader of {}: {leader_failure}",
                machine_path.as_str()
            );
        })
        .ok()?;
    i32::try_from(leader_pid)
        .inspect_err(|conversion_failure| {
            tracing::warn!(
                "machined leader pid {leader_pid} of {} out of range: {conversion_failure}",
                machine_path.as_str()
            );
        })
        .ok()
}

pub(crate) async fn libvirt_leader_pids(system_bus: &zbus::Connection) -> HashSet<i32> {
    let listings = match registered_machine_listings(system_bus).await {
        Ok(listings) => listings,
        Err(listing_error) => {
            tracing::debug!("machined listing: {listing_error}");
            return HashSet::new();
        }
    };
    let mut leader_pids = HashSet::new();
    for machine_path in libvirt_virtual_machine_paths(listings) {
        if let Some(leader_pid) = leader_pid_of(system_bus, machine_path).await {
            leader_pids.insert(leader_pid);
        }
    }
    leader_pids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(
        machine_class: &str,
        registering_service: &str,
        machine_path: &str,
    ) -> RegisteredMachineListing {
        RegisteredMachineListing {
            machine_name: "qemu-1-windows11".to_owned(),
            machine_class: machine_class.to_owned(),
            registering_service: registering_service.to_owned(),
            machine_path: OwnedObjectPath::try_from(machine_path).expect("path is valid"),
        }
    }

    #[test]
    fn only_virtual_machines_registered_by_libvirt_count() {
        let libvirt_machine = "/org/freedesktop/machine1/machine/qemu_2d1";
        let listings = vec![
            listing("vm", "libvirt-qemu", libvirt_machine),
            listing(
                "container",
                "libvirt-lxc",
                "/org/freedesktop/machine1/machine/lxc_2d1",
            ),
            listing(
                "vm",
                "systemd-vmspawn",
                "/org/freedesktop/machine1/machine/vmspawn_2d1",
            ),
        ];
        assert_eq!(
            libvirt_virtual_machine_paths(listings),
            vec![OwnedObjectPath::try_from(libvirt_machine).expect("path is valid")]
        );
    }
}
