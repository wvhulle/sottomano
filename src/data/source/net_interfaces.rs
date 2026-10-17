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
    stream::{self, BoxStream, select},
};
use zbus::{
    Connection, MatchRule, MessageStream,
    fdo::{ManagedObjects, ObjectManagerProxy},
    message::Type,
    names::OwnedInterfaceName,
};

use crate::{
    data::source::{
        Delivery, Source,
        bus::{Bus, Properties, name_owned},
        present_or_absent,
    },
    message_subscriptions::restarting,
};

const SIGNAL_BURST_LIMIT: usize = 64;

#[derive(Debug)]
pub enum ReadingError {
    Bus(zbus::Error),
    Absent,
}

pub fn device_or_absent<S: Source>(
    service: &'static str,
    readings: impl FnMut() -> BoxStream<'static, Option<S>> + Send + 'static,
) -> impl Stream<Item = Delivery<S>> + Send {
    let mut readings = readings;
    present_or_absent(name_owned(Bus::System, service), move || {
        readings()
            .map(|device| device.map_or(Delivery::Absent, |device| Delivery::Reading(Ok(device))))
            .boxed()
    })
}

impl From<zbus::Error> for ReadingError {
    fn from(error: zbus::Error) -> Self {
        Self::Bus(error)
    }
}

pub fn kept_readings<T>(
    label: &'static str,
    readings: impl Stream<Item = Result<T, ReadingError>>,
) -> impl Stream<Item = Option<T>> {
    readings
        .inspect(move |reading| {
            if let Err(ReadingError::Bus(e)) = reading {
                tracing::debug!("{label}: read failed: {e}");
            }
        })
        .filter(|reading| ready(!matches!(reading, Err(ReadingError::Bus(_)))))
        .map(Result::ok)
}

pub fn once_then_per_burst(subscribed_signals: impl Stream<Item = ()>) -> impl Stream<Item = ()> {
    stream::iter([()]).chain(per_burst(subscribed_signals))
}

pub fn per_burst(subscribed_signals: impl Stream<Item = ()>) -> impl Stream<Item = ()> {
    subscribed_signals
        .ready_chunks(SIGNAL_BURST_LIMIT)
        .map(drop)
}

pub fn nm_readings<T, F>(
    label: &'static str,
    watched_properties: &'static [&'static str],
    read: fn(Connection) -> F,
) -> impl Stream<Item = Option<T>>
where
    T: Send + 'static,
    F: Future<Output = Result<T, ReadingError>> + Send + 'static,
{
    restarting(label, move || subscribe_nm(label, watched_properties, read))
}

async fn subscribe_nm<T, F>(
    label: &'static str,
    watched_properties: &'static [&'static str],
    read: fn(Connection) -> F,
) -> zbus::Result<impl Stream<Item = Option<T>>>
where
    T: Send + 'static,
    F: Future<Output = Result<T, ReadingError>> + Send + 'static,
{
    let connection = Connection::system().await?;
    let signals = nm::changes(&connection, watched_properties).await?;
    let readings = once_then_per_burst(signals).then(move |()| read(connection.clone()));
    Ok(kept_readings(label, readings))
}

pub fn managed_objects_readings<T: Send + 'static>(
    label: &'static str,
    service: &'static str,
    path: &'static str,
    reading: fn(&ManagedObjects) -> Option<T>,
) -> impl Stream<Item = Option<T>> {
    restarting(label, move || {
        subscribe_managed_objects(label, service, path, reading)
    })
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
        async move {
            let managed = objects
                .get_managed_objects()
                .await
                .map_err(zbus::Error::from)?;
            reading(&managed).ok_or(ReadingError::Absent)
        }
    });
    Ok(kept_readings(label, readings))
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

pub fn interface<'a>(
    interfaces: &'a HashMap<OwnedInterfaceName, Properties>,
    name: &str,
) -> Option<&'a Properties> {
    interfaces
        .iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, properties)| properties)
}
