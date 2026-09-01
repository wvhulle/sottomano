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

use crate::data::source::{Level, Section, net_interfaces::nm::AddressBlock, row, section};

pub(crate) type Properties = HashMap<String, OwnedValue>;

pub(crate) fn kept_readings<T>(
    label: &'static str,
    readings: impl Stream<Item = zbus::Result<Option<T>>>,
) -> impl Stream<Item = Option<T>> {
    readings.filter_map(move |reading| {
        ready(match reading {
            Ok(reading) => Some(reading),
            Err(e) => {
                tracing::debug!("{label}: read failed: {e}");
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
                    tracing::warn!("{label}: watch failed: {e}");
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

pub(crate) fn address_section(
    key: &impl Fn(&str) -> String,
    label: &str,
    block: Option<&AddressBlock>,
) -> Section {
    let Some(block) = block else {
        return section(key(label), Vec::new());
    };
    let mask_label = if label == "IPv6" {
        "Prefix Length"
    } else {
        "Subnet Mask"
    };
    let mut rows = vec![
        row("Address", block.address.clone()),
        row(mask_label, block.mask.clone()),
    ];
    if let Some(gateway) = &block.gateway {
        rows.push(row("Gateway (Router)", gateway.clone()));
    }
    section(key(label), rows)
}

pub(crate) fn dns_section(key: &impl Fn(&str) -> String, servers: &[String]) -> Section {
    let Some((first, rest)) = servers.split_first() else {
        return section(key("DNS"), Vec::new());
    };
    let mut rows = vec![row("Server", first.clone())];
    rows.extend(rest.iter().map(|server| row("Server", server.clone())));
    section(key("DNS"), rows)
}

pub(crate) fn link_stats_section(
    key: &impl Fn(&str) -> String,
    title: &str,
    rate_label: &str,
    rate: u32,
    mtu: u32,
) -> Section {
    let rows = match (rate, mtu) {
        (0, 0) => Vec::new(),
        (0, mtu) => vec![row("Max Packet Size (MTU)", mtu.to_string())],
        (rate, 0) => vec![row(rate_label, bitrate(rate))],
        (rate, mtu) => vec![
            row(rate_label, bitrate(rate)),
            row("Max Packet Size (MTU)", mtu.to_string()),
        ],
    };
    section(key(title), rows)
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
