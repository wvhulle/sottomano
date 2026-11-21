use std::{
    collections::{HashMap, HashSet},
    future::ready,
};

use futures::{Stream, StreamExt, stream::select};
use sottomano_compositor_model::compositor_window_facts::Urgency;
use zbus::{
    Connection, MatchRule,
    fdo::DBusProxy,
    message::Type,
    zvariant::{OwnedValue, Value},
};

use crate::{
    StateChange,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LauncherProgress {
    Hidden,
    Shown(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherCount {
    Hidden,
    Shown(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LauncherEntry {
    pub progress: LauncherProgress,
    pub count: LauncherCount,
    pub urgency: Urgency,
}

impl LauncherEntry {
    const CLEARED: LauncherEntry = LauncherEntry {
        progress: LauncherProgress::Hidden,
        count: LauncherCount::Hidden,
        urgency: Urgency::NotUrgent,
    };

    fn parse(props: &HashMap<String, OwnedValue>) -> Self {
        let shown_number = |visibility_key: &str, value_key: &str| {
            prop(props, visibility_key)
                .unwrap_or(false)
                .then(|| number(props, value_key))
                .flatten()
        };
        Self {
            progress: shown_number("progress-visible", "progress")
                .map_or(LauncherProgress::Hidden, |value| {
                    LauncherProgress::Shown((value as f32).clamp(0.0, 1.0))
                }),
            count: shown_number("count-visible", "count").map_or(LauncherCount::Hidden, |value| {
                LauncherCount::Shown(value as i64)
            }),
            urgency: if prop(props, "urgent").unwrap_or(false) {
                Urgency::Urgent
            } else {
                Urgency::NotUrgent
            },
        }
    }
}

enum SignalSender {
    Peer(String),
    Unidentified,
}

#[derive(Debug, Clone, Default)]
pub struct LauncherEntries {
    pub by_app: HashMap<String, LauncherEntry>,
}

impl LauncherEntries {}

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
        (registry_change == StateChange::Changed).then(|| self.entries.clone())
    }

    fn update(&mut self, sender: SignalSender, uri: &str, entry: LauncherEntry) -> StateChange {
        let Some(key) = app_key(uri) else {
            return StateChange::Unchanged;
        };
        if let SignalSender::Peer(sender) = sender {
            self.owners.entry(sender).or_default().insert(key.clone());
        }
        let changed = if entry == LauncherEntry::CLEARED {
            self.entries.by_app.remove(&key).is_some()
        } else {
            self.entries.by_app.insert(key, entry.clone()) != Some(entry)
        };
        if changed {
            StateChange::Changed
        } else {
            StateChange::Unchanged
        }
    }

    fn forget_owner(&mut self, sender: &str) -> StateChange {
        let Some(keys) = self.owners.remove(sender) else {
            return StateChange::Unchanged;
        };
        let removed_count = keys
            .iter()
            .filter_map(|key| self.entries.by_app.remove(key))
            .count();
        if removed_count > 0 {
            StateChange::Changed
        } else {
            StateChange::Unchanged
        }
    }
}

enum Signal {
    Update {
        sender: SignalSender,
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
                .inspect_err(|error| tracing::warn!("launcher entry: {error}"))
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
        sender: match message.header().sender() {
            Some(sender) => SignalSender::Peer(sender.to_string()),
            None => SignalSender::Unidentified,
        },
        uri,
        entry: LauncherEntry::parse(&props),
    })
}
