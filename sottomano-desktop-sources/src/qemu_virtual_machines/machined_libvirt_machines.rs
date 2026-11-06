use std::collections::HashSet;

use futures::{Stream, StreamExt, stream};
use zbus::{proxy, zvariant::OwnedObjectPath};

const VIRTUAL_MACHINE_CLASS: &str = "vm";
const LIBVIRT_SERVICE_PREFIX: &str = "libvirt";

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, zbus::zvariant::Type)]
struct RegisteredMachineListing {
    machine_name: String,
    machine_class: String,
    registering_service: String,
    machine_path: OwnedObjectPath,
}

#[proxy(
    interface = "org.freedesktop.machine1.Manager",
    default_service = "org.freedesktop.machine1",
    default_path = "/org/freedesktop/machine1"
)]
trait MachineManager {
    fn list_machines(&self) -> zbus::Result<Vec<RegisteredMachineListing>>;

    #[zbus(signal)]
    fn machine_new(&self, machine: String, path: OwnedObjectPath) -> zbus::Result<()>;

    #[zbus(signal)]
    fn machine_removed(&self, machine: String, path: OwnedObjectPath) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.machine1.Machine",
    default_service = "org.freedesktop.machine1"
)]
trait RegisteredMachine {
    #[zbus(property)]
    fn leader(&self) -> zbus::Result<u32>;
}

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
    let machine = RegisteredMachineProxy::builder(system_bus)
        .path(machine_path)
        .ok()?
        .build()
        .await
        .ok()?;
    i32::try_from(machine.leader().await.ok()?).ok()
}

pub(crate) async fn libvirt_leader_pids(system_bus: &zbus::Connection) -> HashSet<i32> {
    let Ok(manager) = MachineManagerProxy::new(system_bus).await else {
        return HashSet::new();
    };
    let listings = match manager.list_machines().await {
        Ok(listings) => listings,
        Err(failure) => {
            tracing::debug!("machined listing: {failure}");
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

async fn subscribed_machine_set_changes(
    system_bus: zbus::Connection,
) -> zbus::Result<impl Stream<Item = ()> + Send> {
    let manager = MachineManagerProxy::new(&system_bus).await?;
    let registered = manager.receive_machine_new().await?.map(drop);
    let unregistered = manager.receive_machine_removed().await?.map(drop);
    Ok(stream::select(registered, unregistered))
}

pub(crate) fn machine_set_changes(system_bus: zbus::Connection) -> impl Stream<Item = ()> + Send {
    stream::once(subscribed_machine_set_changes(system_bus))
        .filter_map(|subscription| async move {
            subscription
                .inspect_err(|failure| tracing::debug!("machined signals: {failure}"))
                .ok()
        })
        .flatten()
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
