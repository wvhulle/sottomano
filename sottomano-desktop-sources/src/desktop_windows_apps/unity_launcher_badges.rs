use std::{
    collections::{HashMap, HashSet},
    future::ready,
};

use futures::{Stream, StreamExt, stream::select};
use zbus::{Connection, MatchRule, fdo::DBusProxy, message::Type, zvariant::OwnedValue};

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
    let read = |convert: fn(OwnedValue) -> Option<f64>| value.try_clone().ok().and_then(convert);
    read(|v| f64::try_from(v).ok())
        .or_else(|| read(|v| i64::try_from(v).ok().map(|n| n as f64)))
        .or_else(|| read(|v| i32::try_from(v).ok().map(f64::from)))
        .or_else(|| read(|v| u32::try_from(v).ok().map(f64::from)))
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
        .filter_map(|message| async move { update_signal(&message.ok()?) });
    let owners = dbus
        .receive_name_owner_changed()
        .await?
        .filter_map(|signal| async move {
            let args = signal.args().ok()?;
            args.new_owner()
                .is_none()
                .then(|| Signal::OwnerLost(args.name().to_string()))
        });

    connection.request_name(LAUNCHER_NAME).await?;

    Ok(select(updates, owners)
        .scan(Registry::default(), |registry, signal| {
            ready(Some(registry.apply(signal)))
        })
        .filter_map(ready)
        .map(Ok))
}

fn update_signal(message: &zbus::Message) -> Option<Signal> {
    let body = message.body();
    let (uri, props) = body
        .deserialize::<(String, HashMap<String, OwnedValue>)>()
        .ok()?;
    Some(Signal::Update {
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
