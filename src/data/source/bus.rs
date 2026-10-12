use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

use zbus::{
    fdo::DBusProxy,
    names::BusName,
    proxy::{Builder, Defaults},
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::data::empty_text::non_empty;

pub type Properties = HashMap<String, OwnedValue>;

pub fn prop<T: TryFrom<OwnedValue>>(properties: &Properties, key: &str) -> Option<T> {
    properties
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

pub trait DeclaredProperties {
    fn flag(&self, key: &str) -> bool;
    fn boolean(&self, key: &str) -> Option<bool>;
    fn text(&self, key: &str) -> Option<String>;
    fn integer(&self, key: &str) -> Option<i64>;
    fn float(&self, key: &str) -> Option<f64>;
    fn dictionary(&self, key: &str) -> Option<Properties>;
    fn object_path(&self, key: &str) -> Option<OwnedObjectPath>;
}

impl DeclaredProperties for Properties {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Connect(String),
    Call(String),
    Absent,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Connect(detail) => write!(f, "session bus: {detail}"),
            Error::Call(detail) => write!(f, "{detail}"),
            Error::Absent => write!(f, "service not running"),
        }
    }
}

pub async fn session_proxy<P>() -> Result<P, Error>
where
    P: From<zbus::Proxy<'static>> + Defaults,
{
    proxy_on(zbus::Connection::session().await).await
}

pub async fn system_proxy<P>() -> Result<P, Error>
where
    P: From<zbus::Proxy<'static>> + Defaults,
{
    proxy_on(zbus::Connection::system().await).await
}

async fn proxy_on<P>(connection: zbus::Result<zbus::Connection>) -> Result<P, Error>
where
    P: From<zbus::Proxy<'static>> + Defaults,
{
    let connected = |error: zbus::Error| Error::Connect(error.to_string());
    let connection = connection.map_err(connected)?;
    Builder::new(&connection).build().await.map_err(connected)
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

pub fn call<T>(result: zbus::Result<T>) -> Result<T, Error> {
    result.map_err(|error| {
        if service_absent(&error) {
            Error::Absent
        } else {
            Error::Call(error.to_string())
        }
    })
}
