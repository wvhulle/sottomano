pub mod bluez_bluetooth;
pub mod ethernet_connection;
pub mod hid_sysfs_bluetooth_peripherals;
pub mod icmp_echo_probes;
pub mod icmp_gateway_latency;
pub mod icmp_internet_latency;
pub mod interface_throughput;
pub mod modem_manager_mobile;
pub mod network_manager_proxies;
pub mod network_manager_vpn_connections;
pub mod nm_uplink_connectivity;
pub mod proc_tcp_retransmissions;
pub mod sysfs_interface_faults;
pub mod tailscale_status;
pub mod wifi_connection;
pub mod wireguard_tunnels;

use std::{collections::HashMap, future::Future, sync::Arc};

use futures::{Stream, StreamExt, stream::select};
use zbus::{
    Connection, MatchRule, MessageStream,
    fdo::{ManagedObjects, ObjectManagerProxy},
    message::Type,
    names::OwnedInterfaceName,
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{StreamOperators, connected_until_failure, read_once_then_per_burst},
    system_protocol_clients::dbus_desktop_buses::{Properties, name_owned},
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
    name_owned(system, service).while_held(
        move || {
            readings().map(|device| match device {
                Ok(Some(device)) => SnapshotDelivery::live_push(Ok(device)),
                Ok(None) => SnapshotDelivery::SourceUnavailable,
                Err(failure) => SnapshotDelivery::live_push(Err(failure)),
            })
        },
        SnapshotDelivery::SourceUnavailable,
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
    connected_until_failure(subscribe_nm(system, watched_properties, read))
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
    let readings = read_once_then_per_burst(signals, move || read(connection.clone()));
    Ok(device_readings(readings))
}

pub fn managed_objects_readings<T: Send + 'static>(
    system: Connection,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> impl Stream<Item = zbus::Result<Option<T>>> {
    connected_until_failure(subscribe_managed_objects(system, service, path, reading))
}

async fn subscribe_managed_objects<T: Send + 'static>(
    connection: Connection,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> zbus::Result<impl Stream<Item = zbus::Result<Option<T>>>> {
    let objects = Arc::new(ObjectManagerProxy::new(&connection, service, path).await?);
    let signals = select(
        select(
            objects.receive_interfaces_added().await?.map(|_| ()),
            objects.receive_interfaces_removed().await?.map(|_| ()),
        ),
        properties_changed(&connection, service).await?.map(|_| ()),
    );
    let readings = read_once_then_per_burst(signals, move || {
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

pub async fn properties_changed(
    connection: &Connection,
    service: &'static str,
) -> zbus::Result<MessageStream> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(service)?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build();
    MessageStream::for_match_rule(rule, connection, Some(16)).await
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
