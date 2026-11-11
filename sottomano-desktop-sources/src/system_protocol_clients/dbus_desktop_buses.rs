use std::{
    collections::HashMap,
    future::ready,
    hash::{Hash, Hasher},
    pin::pin,
    time::Duration,
};

use futures::{Stream, StreamExt, future::select, stream};
use zbus::{
    fdo::{DBusProxy, IntrospectableProxy},
    names::BusName,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::{
    non_empty_text::non_empty,
    stream_operators::{Availability, resubscribed},
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
    resubscribed(Duration::from_secs(1), connected_until_closed)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
) -> impl Stream<Item = Availability> + Send {
    resubscribed(Duration::from_secs(1), move || {
        owner_changes(connection.clone(), name)
    })
    .map(move |owned| {
        owned.unwrap_or_else(|failure| {
            tracing::warn!("{name}: owner watch: {failure}");
            Availability::Unavailable
        })
    })
}

async fn owner_changes(
    connection: zbus::Connection,
    name: &'static str,
) -> zbus::Result<impl Stream<Item = zbus::Result<Availability>> + Send> {
    let dbus = DBusProxy::new(&connection).await?;
    let owned = if dbus.name_has_owner(BusName::try_from(name)?).await? {
        Availability::Available
    } else {
        Availability::Unavailable
    };
    let changes = dbus
        .receive_name_owner_changed_with_args(&[(0, name)])
        .await?
        .filter_map(|signal| {
            ready(
                signal
                    .args()
                    .ok()
                    .map(|args| match args.new_owner().as_ref() {
                        Some(_) => Availability::Available,
                        None => Availability::Unavailable,
                    }),
            )
        });
    Ok(stream::iter([owned]).chain(changes).map(Ok))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterfaceOffer {
    ExpectedInterfaceOffered,
    OtherVersionsOffered {
        expected_interface: String,
        offered_versions: Vec<String>,
    },
}

#[must_use]
pub fn interface_offer_among<'names>(
    expected_interface: &str,
    offered_interface_names: impl IntoIterator<Item = &'names str>,
) -> InterfaceOffer {
    let interface_family =
        expected_interface.trim_end_matches(|character: char| character.is_ascii_digit());
    let mut offered_versions = Vec::new();
    for offered_name in offered_interface_names {
        if offered_name == expected_interface {
            return InterfaceOffer::ExpectedInterfaceOffered;
        }
        let same_family =
            offered_name
                .strip_prefix(interface_family)
                .is_some_and(|version_suffix| {
                    version_suffix
                        .chars()
                        .all(|character| character.is_ascii_digit())
                });
        if same_family {
            offered_versions.push(offered_name.to_owned());
        }
    }
    InterfaceOffer::OtherVersionsOffered {
        expected_interface: expected_interface.to_owned(),
        offered_versions,
    }
}

pub async fn introspected_interface_offer(
    connection: &zbus::Connection,
    bus_name: &'static str,
    object_path: &'static str,
    expected_interface: &str,
) -> Result<InterfaceOffer, Error> {
    let introspection = IntrospectableProxy::builder(connection)
        .destination(bus_name)?
        .path(object_path)?
        .build()
        .await?;
    let introspection_xml = introspection
        .introspect()
        .await
        .map_err(zbus::Error::from)?;
    let object_node = zbus_xml::Node::from_reader(introspection_xml.as_bytes())
        .map_err(|parse_error| Error::Call(format!("{bus_name}: introspection: {parse_error}")))?;
    let offered_names: Vec<String> = object_node
        .interfaces()
        .iter()
        .map(|interface| interface.name().to_string())
        .collect();
    Ok(interface_offer_among(
        expected_interface,
        offered_names.iter().map(String::as_str),
    ))
}

pub const SERVICE_ABSENT_ERRORS: [&str; 3] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.NoReply",
];

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::MethodError(name, _, _)
                if SERVICE_ABSENT_ERRORS.contains(&name.as_str()) =>
            {
                Error::Absent
            }
            _ => Error::Call(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_DICTATION: &str = "page.example.Dictation4";

    #[test]
    fn exact_interface_name_is_offered() {
        assert_eq!(
            interface_offer_among(
                EXPECTED_DICTATION,
                ["org.freedesktop.DBus.Properties", "page.example.Dictation4"],
            ),
            InterfaceOffer::ExpectedInterfaceOffered
        );
    }

    #[test]
    fn other_version_of_the_family_is_reported() {
        assert_eq!(
            interface_offer_among(
                EXPECTED_DICTATION,
                [
                    "org.freedesktop.DBus.Introspectable",
                    "page.example.Dictation5",
                    "page.example.DictationExtras",
                ],
            ),
            InterfaceOffer::OtherVersionsOffered {
                expected_interface: EXPECTED_DICTATION.to_owned(),
                offered_versions: vec!["page.example.Dictation5".to_owned()],
            }
        );
    }

    #[test]
    fn missing_family_reports_no_offered_versions() {
        assert_eq!(
            interface_offer_among(EXPECTED_DICTATION, ["org.freedesktop.DBus.Peer"]),
            InterfaceOffer::OtherVersionsOffered {
                expected_interface: EXPECTED_DICTATION.to_owned(),
                offered_versions: Vec::new(),
            }
        );
    }
}
