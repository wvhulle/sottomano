pub mod bluetooth;
pub mod ethernet;
pub mod mobile;
pub mod nm;
pub mod wifi;

use std::{
    collections::HashMap,
    future::{Future, ready},
    sync::Arc,
};

use bytesize::ByteSize;
use iced::futures::{
    Stream, StreamExt,
    stream::{self, select},
};
use zbus::{
    Connection, MatchRule, MessageStream,
    fdo::{ManagedObjects, ObjectManagerProxy},
    message::Type,
    names::OwnedInterfaceName,
    zvariant::OwnedValue,
};

use crate::data::source::{Level, Row, net_interfaces::nm::AddressBlock, row, separator};

pub(crate) type Properties = HashMap<String, OwnedValue>;

pub(crate) fn kept_readings<T>(
    label: &'static str,
    readings: impl Stream<Item = zbus::Result<Option<T>>>,
) -> impl Stream<Item = Option<T>> {
    readings.filter_map(move |reading| {
        ready(match reading {
            Ok(reading) => Some(reading),
            Err(e) => {
                eprintln!("{label}: read failed: {e}");
                None
            }
        })
    })
}

pub(crate) fn once_then_on_each(
    subscribed_signals: impl Stream<Item = ()>,
) -> impl Stream<Item = ()> {
    stream::once(ready(())).chain(subscribed_signals)
}

fn once_subscribed<T, S>(
    label: &'static str,
    subscribe: impl Future<Output = zbus::Result<S>>,
) -> impl Stream<Item = Option<T>>
where
    S: Stream<Item = Option<T>>,
{
    stream::once(subscribe)
        .filter_map(move |subscribed| {
            ready(match subscribed {
                Ok(readings) => Some(readings),
                Err(e) => {
                    eprintln!("{label}: watch failed: {e}");
                    None
                }
            })
        })
        .flatten()
}

pub(crate) fn nm_readings<T, F>(
    label: &'static str,
    read: fn(Connection) -> F,
) -> impl Stream<Item = Option<T>>
where
    T: Send + 'static,
    F: Future<Output = zbus::Result<Option<T>>> + Send + 'static,
{
    once_subscribed(label, subscribe_nm(label, read))
}

async fn subscribe_nm<T, F>(
    label: &'static str,
    read: fn(Connection) -> F,
) -> zbus::Result<impl Stream<Item = Option<T>>>
where
    T: Send + 'static,
    F: Future<Output = zbus::Result<Option<T>>> + Send + 'static,
{
    let connection = Connection::system().await?;
    let signals = nm::changes(&connection).await?;
    let readings = once_then_on_each(signals).then(move |()| read(connection.clone()));
    Ok(kept_readings(label, readings))
}

pub(crate) fn managed_objects_readings<T: Send + 'static>(
    label: &'static str,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> impl Stream<Item = Option<T>> {
    once_subscribed(
        label,
        subscribe_managed_objects(label, service, path, reading),
    )
}

async fn subscribe_managed_objects<T: Send + 'static>(
    label: &'static str,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> zbus::Result<impl Stream<Item = Option<T>>> {
    let connection = Connection::system().await?;
    let objects = Arc::new(ObjectManagerProxy::new(&connection, service, path).await?);
    let signals = select(
        select(
            objects.receive_interfaces_added().await?.map(|_| ()),
            objects.receive_interfaces_removed().await?.map(|_| ()),
        ),
        properties_changed(&connection, service).await?.map(|_| ()),
    );
    let readings = once_then_on_each(signals).then(move |()| {
        let objects = objects.clone();
        async move { Ok(reading(&objects.get_managed_objects().await?)) }
    });
    Ok(kept_readings(label, readings))
}

pub(crate) async fn properties_changed(
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

pub(crate) fn group(rows: &mut Vec<Row>, section: Vec<Row>) {
    if section.is_empty() {
        return;
    }
    if !rows.is_empty() {
        rows.push(separator());
    }
    rows.extend(section);
}

pub(crate) fn address_rows(
    key: &impl Fn(&str) -> String,
    label: &str,
    block: Option<&AddressBlock>,
) -> Vec<Row> {
    let Some(block) = block else {
        return Vec::new();
    };
    let mask_label = if label == "IPv6" { "Prefix" } else { "Netmask" };
    let mut rows = vec![
        row(&key(label), block.address.clone()),
        row(&key(mask_label), block.mask.clone()),
    ];
    if let Some(gateway) = &block.gateway {
        rows.push(row(&key("Gateway"), gateway.clone()));
    }
    rows
}

pub(crate) fn dns_rows(key: &impl Fn(&str) -> String, servers: &[String]) -> Vec<Row> {
    let Some((first, rest)) = servers.split_first() else {
        return Vec::new();
    };
    let mut rows = vec![row(&key("DNS"), first.clone())];
    rows.extend(rest.iter().map(|server| row("", server.clone())));
    rows
}

pub(crate) fn link_stats_rows(
    key: &impl Fn(&str) -> String,
    rate_label: &str,
    rate: u32,
    mtu: u32,
) -> Vec<Row> {
    match (rate, mtu) {
        (0, 0) => Vec::new(),
        (0, mtu) => vec![row(&key("MTU"), mtu.to_string())],
        (rate, 0) => vec![row(&key(rate_label), bitrate(rate))],
        (rate, mtu) => vec![
            row(&key(rate_label), bitrate(rate)),
            row("MTU", mtu.to_string()),
        ],
    }
}

fn bitrate(megabits_per_sec: u32) -> String {
    let bits_per_sec = u64::from(megabits_per_sec) * 1_000_000;
    format!("{}/s", ByteSize::b(bits_per_sec / 8).display().si_bits())
}

pub(crate) fn interface<'a>(
    interfaces: &'a HashMap<OwnedInterfaceName, Properties>,
    name: &str,
) -> Option<&'a Properties> {
    interfaces
        .iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, properties)| properties)
}

pub(crate) fn prop<T: TryFrom<OwnedValue>>(properties: &Properties, key: &str) -> Option<T> {
    properties
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

pub(crate) fn signal_level(quality: i64) -> Level {
    match quality {
        ..10 => Level::Critical,
        10..25 => Level::Warning,
        _ => Level::Ok,
    }
}
