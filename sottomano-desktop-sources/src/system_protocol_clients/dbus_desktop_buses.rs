use std::{
    collections::HashMap,
    future::ready,
    hash::{Hash, Hasher},
    pin::pin,
};

use futures::{Stream, StreamExt, future::select, stream};
use zbus::{
    fdo::DBusProxy,
    names::BusName,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::{
    non_empty_text::non_empty,
    stream_operators::{connected_until_failure, resubscribing_until_failure},
};

pub type Properties = HashMap<String, OwnedValue>;

#[must_use]
pub fn prop<T: TryFrom<OwnedValue>>(properties: &Properties, key: &str) -> Option<T> {
    properties
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

pub trait DbusPropertyLookup {
    fn flag(&self, key: &str) -> bool;
    fn boolean(&self, key: &str) -> Option<bool>;
    fn text(&self, key: &str) -> Option<String>;
    fn integer(&self, key: &str) -> Option<i64>;
    fn float(&self, key: &str) -> Option<f64>;
    fn dictionary(&self, key: &str) -> Option<Properties>;
    fn object_path(&self, key: &str) -> Option<OwnedObjectPath>;
}

impl DbusPropertyLookup for Properties {
    fn flag(&self, key: &str) -> bool {
        self.boolean(key).unwrap_or(false)
    }

    fn boolean(&self, key: &str) -> Option<bool> {
        bool::try_from(self.get(key)?).ok()
    }

    fn text(&self, key: &str) -> Option<String> {
        non_empty(prop(self, key)?)
    }

    fn integer(&self, key: &str) -> Option<i64> {
        i64::try_from(self.get(key)?).ok()
    }

    fn float(&self, key: &str) -> Option<f64> {
        f64::try_from(self.get(key)?).ok()
    }

    fn dictionary(&self, key: &str) -> Option<Properties> {
        prop(self, key)
    }

    fn object_path(&self, key: &str) -> Option<OwnedObjectPath> {
        prop(self, key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("session bus: {0}")]
    Connect(String),
    #[error("{0}")]
    Call(String),
    #[error("service not running")]
    Absent,
}

#[derive(Debug, Clone)]
pub struct DesktopBuses {
    pub session: zbus::Connection,
    pub system: zbus::Connection,
}

impl DesktopBuses {
    pub async fn connect() -> zbus::Result<Self> {
        Ok(Self {
            session: zbus::Connection::session().await?,
            system: zbus::Connection::system().await?,
        })
    }

    pub fn on(&self, bus: Bus) -> &zbus::Connection {
        match bus {
            Bus::Session => &self.session,
            Bus::System => &self.system,
        }
    }

    async fn closed(&self) {
        let session = pin!(self.session.closed());
        let system = pin!(self.system.closed());
        select(session, system).await;
    }

    fn unique_names(&self) -> [Option<&str>; 2] {
        [&self.session, &self.system]
            .map(|connection| connection.unique_name().map(|name| name.as_str()))
    }
}

impl PartialEq for DesktopBuses {
    fn eq(&self, other: &Self) -> bool {
        self.unique_names() == other.unique_names()
    }
}

impl Eq for DesktopBuses {}

impl Hash for DesktopBuses {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unique_names().hash(state);
    }
}

#[derive(Debug, Clone)]
pub enum DesktopBusEvent {
    Connected(DesktopBuses),
    Disconnected,
}

pub fn desktop_bus_events() -> impl Stream<Item = Result<DesktopBusEvent, zbus::Error>> + Send {
    resubscribing_until_failure(connected_until_closed)
}

async fn connected_until_closed()
-> zbus::Result<impl Stream<Item = zbus::Result<DesktopBusEvent>> + Send> {
    let buses = DesktopBuses::connect().await?;
    let connected = DesktopBusEvent::Connected(buses.clone());
    let disconnected = async move {
        buses.closed().await;
        DesktopBusEvent::Disconnected
    };
    Ok(stream::iter([connected])
        .chain(stream::once(disconnected))
        .map(Ok))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OwnerProcess {
    Identified(u32),
    #[default]
    Unidentified,
}

impl OwnerProcess {
    pub async fn of_bus(dbus: &DBusProxy<'_>, bus: BusName<'_>) -> Self {
        let described = bus.to_string();
        match dbus.get_connection_unix_process_id(bus).await {
            Ok(pid) => Self::Identified(pid),
            Err(e) => {
                tracing::debug!("{described}: no owner process: {e}");
                Self::Unidentified
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    Session,
    System,
}

pub fn name_owned(
    connection: zbus::Connection,
    name: &'static str,
) -> impl Stream<Item = bool> + Send {
    connected_until_failure(owner_changes(connection, name)).map(move |owned| {
        owned.unwrap_or_else(|e| {
            tracing::warn!("{name}: owner watch: {e}");
            false
        })
    })
}

async fn owner_changes(
    connection: zbus::Connection,
    name: &'static str,
) -> zbus::Result<impl Stream<Item = zbus::Result<bool>> + Send> {
    let dbus = DBusProxy::new(&connection).await?;
    let owned = dbus.name_has_owner(BusName::try_from(name)?).await?;
    let changes = dbus
        .receive_name_owner_changed_with_args(&[(0, name)])
        .await?
        .filter_map(|signal| ready(signal.args().ok().map(|args| args.new_owner().is_some())));
    Ok(stream::iter([owned]).chain(changes).map(Ok))
}

#[must_use]
pub fn service_absent(error: &zbus::Error) -> bool {
    let zbus::Error::MethodError(name, _, _) = error else {
        return false;
    };
    matches!(
        name.as_str(),
        "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.NoReply"
    )
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        if service_absent(&error) {
            Error::Absent
        } else {
            Error::Call(error.to_string())
        }
    }
}
