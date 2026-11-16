pub mod bluez_bluetooth;
pub mod ethernet_connection;
pub mod hid_sysfs_bluetooth_peripherals;
pub mod icmp_echo_probes;
pub mod icmp_gateway_latency;
pub mod icmp_internet_latency;
pub mod interface_bus_attachment;
pub mod interface_throughput;
pub mod internet_connectivity_gate;
pub mod modem_manager_mobile;
pub mod nearby_wifi_networks;
pub mod network_manager_proxies;
pub mod network_manager_vpn_connections;
pub mod nm_uplink_connectivity;
pub mod sysfs_interface_faults;
pub mod tailscale_exit_nodes;
pub mod tailscale_inbound_access;
pub mod tailscale_policy_file;
pub mod tailscale_status;
pub mod wifi_connection;
pub mod wireguard_tunnels;

use std::{collections::HashMap, future::Future, sync::Arc};

use futures::{Stream, StreamExt};
use zbus::{
    Connection,
    fdo::{ManagedObjects, ObjectManagerProxy},
    names::OwnedInterfaceName,
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::ChangeTriggerStreams,
    system_protocol_clients::{
        dbus_change_signals::object_manager_changes,
        dbus_desktop_buses::{BusReadError, Properties, name_owned},
        protocol_peer_reconnects::subscribed,
    },
};

pub fn device_or_absent<S, R>(
    system: zbus::Connection,
    service: &'static str,
    mut samples: impl FnMut() -> R + Send,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource<Error = BusReadError>,
    R: Stream<Item = Result<S, BusReadError>> + Send,
{
    name_owned(system, service).while_available(
        move || {
            samples().map(|device| SnapshotDelivery::live_push(device).unavailable_when_absent())
        },
        || SnapshotDelivery::SourceUnavailable,
    )
}

pub fn nm_samples<T, F>(
    system: Connection,
    watched_properties: &'static [&'static str],
    read: fn(Connection) -> F,
) -> impl Stream<Item = Result<T, BusReadError>>
where
    T: Send + 'static,
    F: Future<Output = Result<T, BusReadError>> + Send + 'static,
{
    subscribed(system, move |system| {
        subscribe_nm(system, watched_properties, read)
    })
}

async fn subscribe_nm<T, F>(
    connection: Connection,
    watched_properties: &'static [&'static str],
    read: fn(Connection) -> F,
) -> Result<impl Stream<Item = Result<T, BusReadError>>, BusReadError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, BusReadError>> + Send + 'static,
{
    let signals = network_manager_proxies::changes(&connection, watched_properties).await?;
    Ok(signals.triggered_rereads(move || read(connection.clone())))
}

pub fn managed_objects_samples<T: Send + 'static>(
    system: Connection,
    service: &'static str,
    path: &'static str,
    sample: fn(&ManagedObjects) -> Option<T>,
) -> impl Stream<Item = Result<T, BusReadError>> {
    subscribed(system, move |system| {
        subscribe_managed_objects(system, service, path, sample)
    })
}

async fn subscribe_managed_objects<T: Send + 'static>(
    connection: Connection,
    service: &'static str,
    path: &'static str,
    sample: fn(&ManagedObjects) -> Option<T>,
) -> Result<impl Stream<Item = Result<T, BusReadError>>, BusReadError> {
    let objects = Arc::new(ObjectManagerProxy::new(&connection, service, path).await?);
    let signals = object_manager_changes(&connection, service, path).await?;
    Ok(signals.triggered_rereads(move || {
        let objects = objects.clone();
        async move {
            let managed = objects.get_managed_objects().await?;
            sample(&managed).ok_or(BusReadError::Absent)
        }
    }))
}

#[must_use]
pub fn interface<'a>(
    interfaces: &'a HashMap<OwnedInterfaceName, Properties>,
    name: &str,
) -> Option<&'a Properties> {
    interfaces
        .iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, properties)| properties)
}
