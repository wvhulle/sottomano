use std::{
    collections::{HashMap, HashSet},
    future::ready,
};

use futures::{Stream, StreamExt, stream::select};
use zbus::{
    Connection, MatchRule,
    fdo::DBusProxy,
    message::Type,
    zvariant::{OwnedValue, Value},
};

use crate::{
    non_empty_text::non_empty,
    system_protocol_clients::{
        dbus_change_signals::signal_messages,
        dbus_desktop_buses::{DesktopBuses, prop},
        protocol_peer_reconnects::subscribed,
    },
};

const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";
const UPDATE: &str = "Update";
const LAUNCHER_NAME: &str = "com.canonical.Unity";
const URI_PREFIX: &str = "application://";
const DESKTOP_SUFFIX: &str = ".desktop";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LauncherEntry {
    pub progress: Option<f32>,
    pub count: Option<i64>,
    pub urgent: bool,
}

impl LauncherEntry {
    fn parse(props: &HashMap<String, OwnedValue>) -> Self {
        Self {
            progress: prop(props, "progress-visible")
                .unwrap_or(false)
                .then(|| number(props, "progress"))
                .flatten()
                .map(|value| (value as f32).clamp(0.0, 1.0)),
            count: prop(props, "count-visible")
                .unwrap_or(false)
                .then(|| number(props, "count"))
                .flatten()
                .map(|value| value as i64),
            urgent: prop(props, "urgent").unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LauncherEntries {
    by_app: HashMap<String, LauncherEntry>,
}

impl LauncherEntries {
    #[must_use]
    pub fn get(&self, app_id: &str) -> Option<&LauncherEntry> {
        self.by_app.get(app_id)
    }
}

#[allow(clippy::cast_precision_loss)]
fn number(props: &HashMap<String, OwnedValue>, key: &str) -> Option<f64> {
    let value = props.get(key)?;
    let converted_number = match &**value {
        Value::F64(float_number) => Some(*float_number),
        Value::I64(signed_number) => Some(*signed_number as f64),
        Value::I32(signed_number) => Some(f64::from(*signed_number)),
        Value::U32(unsigned_number) => Some(f64::from(*unsigned_number)),
        _ => None,
    };
    if converted_number.is_none() {
        tracing::warn!("launcher entry: {key} is not a number: {value:?}");
    }
    converted_number
}

fn app_key(uri: &str) -> Option<String> {
    let name = uri.strip_prefix(URI_PREFIX)?;
    non_empty(name.strip_suffix(DESKTOP_SUFFIX).unwrap_or(name).to_owned())
}

#[derive(Default)]
struct Registry {
    entries: LauncherEntries,
    owners: HashMap<String, HashSet<String>>,
}

impl Registry {
    fn apply(&mut self, signal: Signal) -> Option<LauncherEntries> {
        let registry_change = match signal {
            Signal::Update { sender, uri, entry } => self.update(sender, &uri, entry),
            Signal::OwnerLost(sender) => self.forget_owner(&sender),
        };
        (registry_change == RegistryChange::Changed).then(|| self.entries.clone())
    }

    fn update(
        &mut self,
        sender: Option<String>,
        uri: &str,
        entry: LauncherEntry,
    ) -> RegistryChange {
        let Some(key) = app_key(uri) else {
            return RegistryChange::Unchanged;
        };
        if let Some(sender) = sender {
            self.owners.entry(sender).or_default().insert(key.clone());
        }
        let changed = if entry == LauncherEntry::default() {
            self.entries.by_app.remove(&key).is_some()
        } else {
            self.entries.by_app.insert(key, entry.clone()) != Some(entry)
        };
        if changed {
            RegistryChange::Changed
        } else {
            RegistryChange::Unchanged
        }
    }

    fn forget_owner(&mut self, sender: &str) -> RegistryChange {
        let Some(keys) = self.owners.remove(sender) else {
            return RegistryChange::Unchanged;
        };
        let removed_count = keys
            .iter()
            .filter_map(|key| self.entries.by_app.remove(key))
            .count();
        if removed_count > 0 {
            RegistryChange::Changed
        } else {
            RegistryChange::Unchanged
        }
    }
}

enum Signal {
    Update {
        sender: Option<String>,
        uri: String,
        entry: LauncherEntry,
    },
    OwnerLost(String),
}

pub fn entry_updates(buses: &DesktopBuses) -> impl Stream<Item = LauncherEntries> + Send + use<> {
    let session = buses.session.clone();
    subscribed(session, connect).filter_map(|entries| {
        ready(
            entries
                .inspect_err(|e| tracing::warn!("launcher entry: {e}"))
                .ok(),
        )
    })
}

async fn connect(
    connection: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<LauncherEntries>>> {
    let dbus = DBusProxy::new(&connection).await?;

    let update_rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(INTERFACE)?
        .member(UPDATE)?
        .build();
    let updates = signal_messages(&connection, update_rule)
        .await?
        .map(|message| message.and_then(|message| update_signal(&message)));
    let owners = dbus
        .receive_name_owner_changed()
        .await?
        .filter_map(|signal| {
            ready(
                signal
                    .args()
                    .map(|args| {
                        args.new_owner()
                            .is_none()
                            .then(|| Signal::OwnerLost(args.name().to_string()))
                    })
                    .transpose(),
            )
        });

    connection.request_name(LAUNCHER_NAME).await?;

    Ok(select(updates, owners)
        .scan(Registry::default(), |registry, signal| {
            ready(Some(signal.map(|signal| registry.apply(signal))))
        })
        .filter_map(|applied| ready(applied.transpose())))
}

fn update_signal(message: &zbus::Message) -> zbus::Result<Signal> {
    let body = message.body();
    let (uri, props) = body.deserialize::<(String, HashMap<String, OwnedValue>)>()?;
    Ok(Signal::Update {
        sender: message.header().sender().map(ToString::to_string),
        uri,
        entry: LauncherEntry::parse(&props),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistryChange {
    Changed,
    Unchanged,
}
