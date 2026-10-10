use std::fmt::{Display, Formatter, Result as FmtResult};

use zbus::{fdo::DBusProxy, names::BusName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Connect(String),
    Call(String),
    Absent,
    Panicked,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Connect(detail) => write!(f, "session bus: {detail}"),
            Error::Call(detail) => write!(f, "{detail}"),
            Error::Absent => write!(f, "service not running"),
            Error::Panicked => write!(f, "bus call panicked"),
        }
    }
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
