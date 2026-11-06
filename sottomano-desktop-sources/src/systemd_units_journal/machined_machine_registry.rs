use futures::{Stream, StreamExt, stream};
use zbus::{proxy, zvariant::OwnedObjectPath};

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, zbus::zvariant::Type)]
pub(crate) struct RegisteredMachineListing {
    pub(crate) machine_name: String,
    pub(crate) machine_class: String,
    pub(crate) registering_service: String,
    pub(crate) machine_path: OwnedObjectPath,
}

#[proxy(
    interface = "org.freedesktop.machine1.Manager",
    default_service = "org.freedesktop.machine1",
    default_path = "/org/freedesktop/machine1"
)]
pub(crate) trait MachineManager {
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
pub(crate) trait RegisteredMachine {
    fn get_addresses(&self) -> zbus::Result<Vec<(i32, Vec<u8>)>>;

    #[zbus(property)]
    fn leader(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn unit(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn timestamp(&self) -> zbus::Result<u64>;

    #[zbus(property)]
    fn root_directory(&self) -> zbus::Result<String>;
}

pub(crate) async fn registered_machine_at(
    system_bus: &zbus::Connection,
    machine_path: OwnedObjectPath,
) -> zbus::Result<RegisteredMachineProxy<'static>> {
    RegisteredMachineProxy::builder(system_bus)
        .path(machine_path)?
        .build()
        .await
}

pub(crate) async fn registered_machine_listings(
    system_bus: &zbus::Connection,
) -> zbus::Result<Vec<RegisteredMachineListing>> {
    MachineManagerProxy::new(system_bus)
        .await?
        .list_machines()
        .await
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
                .inspect_err(|subscription_error| {
                    tracing::debug!("machined signals: {subscription_error}");
                })
                .ok()
        })
        .flatten()
}
