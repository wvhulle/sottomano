use std::collections::HashSet;

use zbus::zvariant::OwnedObjectPath;

use crate::systemd_units_journal::machined_machine_registry::{
    RegisteredMachineListing, registered_machine_at, registered_machine_listings,
};

const VIRTUAL_MACHINE_CLASS: &str = "vm";
const LIBVIRT_SERVICE_PREFIX: &str = "libvirt";

fn is_libvirt_virtual_machine(listing: &RegisteredMachineListing) -> bool {
    listing.machine_class == VIRTUAL_MACHINE_CLASS
        && listing
            .registering_service
            .starts_with(LIBVIRT_SERVICE_PREFIX)
}

async fn leader_pid_of(
    system_bus: &zbus::Connection,
    machine_path: OwnedObjectPath,
) -> Option<i32> {
    let machine = registered_machine_at(system_bus, machine_path).await.ok()?;
    i32::try_from(machine.leader().await.ok()?).ok()
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
    for listing in listings.into_iter().filter(is_libvirt_virtual_machine) {
        if let Some(leader_pid) = leader_pid_of(system_bus, listing.machine_path).await {
            leader_pids.insert(leader_pid);
        }
    }
    leader_pids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(machine_class: &str, registering_service: &str) -> RegisteredMachineListing {
        RegisteredMachineListing {
            machine_name: "qemu-1-windows11".to_owned(),
            machine_class: machine_class.to_owned(),
            registering_service: registering_service.to_owned(),
            machine_path: OwnedObjectPath::try_from("/org/freedesktop/machine1/machine/qemu_2d1")
                .expect("path is valid"),
        }
    }

    #[test]
    fn only_virtual_machines_registered_by_libvirt_count() {
        assert!(is_libvirt_virtual_machine(&listing("vm", "libvirt-qemu")));
        assert!(!is_libvirt_virtual_machine(&listing(
            "container",
            "libvirt-lxc"
        )));
        assert!(!is_libvirt_virtual_machine(&listing(
            "vm",
            "systemd-vmspawn"
        )));
    }
}
