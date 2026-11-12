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
        dbus_desktop_buses::{Properties, name_owned},
        protocol_peer_reconnects::subscribed,
    },
};

#[derive(Debug)]
pub enum ReadingError {
    Bus(zbus::Error),
    Absent,
}

pub fn device_or_absent<S, R>(
    system: zbus::Connection,
    service: &'static str,
    mut readings: impl FnMut() -> R + Send,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource<Error = zbus::Error>,
    R: Stream<Item = zbus::Result<Option<S>>> + Send,
{
    name_owned(system, service).while_available(
        move || {
            readings().map(|device| match device {
                Ok(Some(device)) => SnapshotDelivery::live_push(Ok(device)),
                Ok(None) => SnapshotDelivery::SourceUnavailable,
                Err(failure) => SnapshotDelivery::live_push(Err(failure)),
            })
        },
        || SnapshotDelivery::SourceUnavailable,
    )
}

impl From<zbus::Error> for ReadingError {
    fn from(error: zbus::Error) -> Self {
        Self::Bus(error)
    }
}

pub fn device_readings<T>(
    readings: impl Stream<Item = Result<T, ReadingError>>,
) -> impl Stream<Item = zbus::Result<Option<T>>> {
    readings.map(|reading| match reading {
        Ok(device) => Ok(Some(device)),
        Err(ReadingError::Absent) => Ok(None),
        Err(ReadingError::Bus(failure)) => Err(failure),
    })
}

pub fn nm_readings<T, F>(
    system: Connection,
    watched_properties: &'static [&'static str],
    read: fn(Connection) -> F,
) -> impl Stream<Item = zbus::Result<Option<T>>>
where
    T: Send + 'static,
    F: Future<Output = Result<T, ReadingError>> + Send + 'static,
{
    subscribed(system, move |system| {
        subscribe_nm(system, watched_properties, read)
    })
}

async fn subscribe_nm<T, F>(
    connection: Connection,
    watched_properties: &'static [&'static str],
    read: fn(Connection) -> F,
) -> zbus::Result<impl Stream<Item = zbus::Result<Option<T>>>>
where
    T: Send + 'static,
    F: Future<Output = Result<T, ReadingError>> + Send + 'static,
{
    let signals = network_manager_proxies::changes(&connection, watched_properties).await?;
    let readings = signals.triggered_rereads(move || read(connection.clone()));
    Ok(device_readings(readings))
}

pub fn managed_objects_readings<T: Send + 'static>(
    system: Connection,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> impl Stream<Item = zbus::Result<Option<T>>> {
    subscribed(system, move |system| {
        subscribe_managed_objects(system, service, path, reading)
    })
}

async fn subscribe_managed_objects<T: Send + 'static>(
    connection: Connection,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> zbus::Result<impl Stream<Item = zbus::Result<Option<T>>>> {
    let objects = Arc::new(ObjectManagerProxy::new(&connection, service, path).await?);
    let signals = object_manager_changes(&connection, service, path).await?;
    let readings = signals.triggered_rereads(move || {
        let objects = objects.clone();
        async move {
            let managed = objects
                .get_managed_objects()
                .await
                .map_err(zbus::Error::from)?;
            reading(&managed).ok_or(ReadingError::Absent)
        }
    });
    Ok(device_readings(readings))
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
