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

pub(crate) type Properties = HashMap<String, OwnedValue>;

const SIGNAL_BURST_LIMIT: usize = 64;

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

pub(crate) fn once_then_per_burst(
    subscribed_signals: impl Stream<Item = ()>,
) -> impl Stream<Item = ()> {
    stream::once(ready(())).chain(
        subscribed_signals
            .ready_chunks(SIGNAL_BURST_LIMIT)
            .map(|_| ()),
    )
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
    let readings = once_then_per_burst(signals).then(move |()| read(connection.clone()));
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
    let readings = once_then_per_burst(signals).then(move |()| {
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
