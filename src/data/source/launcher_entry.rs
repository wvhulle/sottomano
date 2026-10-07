use std::{
    collections::{HashMap, HashSet},
    future::ready,
};

use iced::{
    Subscription,
    futures::{Stream, StreamExt, stream::select},
};
use zbus::{
    Connection, MatchRule, MessageStream, fdo::DBusProxy, message::Type, zvariant::OwnedValue,
};

use crate::message_subscriptions::restarting;

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
            progress: visible(props, "progress-visible")
                .then(|| number(props, "progress"))
                .flatten()
                .map(|value| (value as f32).clamp(0.0, 1.0)),
            count: visible(props, "count-visible")
                .then(|| number(props, "count"))
                .flatten()
                .map(|value| value as i64),
            urgent: visible(props, "urgent"),
        }
    }

    fn is_empty(&self) -> bool {
        self.progress.is_none() && self.count.is_none() && !self.urgent
    }
}

#[derive(Debug, Clone, Default)]
pub struct LauncherEntries {
    by_app: HashMap<String, LauncherEntry>,
}

impl LauncherEntries {
    pub fn get(&self, app_id: &str) -> Option<&LauncherEntry> {
        self.by_app.get(app_id)
    }
}

fn visible(props: &HashMap<String, OwnedValue>, key: &str) -> bool {
    props
        .get(key)
        .and_then(|value| bool::try_from(value.try_clone().ok()?).ok())
        .unwrap_or(false)
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
    let name = name.strip_suffix(DESKTOP_SUFFIX).unwrap_or(name);
    (!name.is_empty()).then(|| name.to_owned())
}

#[derive(Default)]
struct Registry {
    entries: LauncherEntries,
    owners: HashMap<String, HashSet<String>>,
}

impl Registry {
    fn apply(&mut self, signal: Signal) -> Option<LauncherEntries> {
        let changed = match signal {
            Signal::Update { sender, uri, entry } => self.update(sender, &uri, entry),
            Signal::OwnerLost(sender) => self.forget_owner(&sender),
        };
        changed.then(|| self.entries.clone())
    }

    fn update(&mut self, sender: Option<String>, uri: &str, entry: LauncherEntry) -> bool {
        let Some(key) = app_key(uri) else {
            return false;
        };
        if let Some(sender) = sender {
            self.owners.entry(sender).or_default().insert(key.clone());
        }
        if entry.is_empty() {
            return self.entries.by_app.remove(&key).is_some();
        }
        self.entries.by_app.insert(key, entry.clone()) != Some(entry)
    }

    fn forget_owner(&mut self, sender: &str) -> bool {
        let Some(keys) = self.owners.remove(sender) else {
            return false;
        };
        keys.iter()
            .filter_map(|key| self.entries.by_app.remove(key))
            .count()
            > 0
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

pub fn subscription() -> Subscription<LauncherEntries> {
    Subscription::run(entries)
}

fn entries() -> impl Stream<Item = LauncherEntries> {
    restarting("launcher entry", connect)
}

async fn connect() -> zbus::Result<impl Stream<Item = LauncherEntries>> {
    let connection = Connection::session().await?;
    let dbus = DBusProxy::new(&connection).await?;

    let update_rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(INTERFACE)?
        .member(UPDATE)?
        .build();
    let updates = MessageStream::for_match_rule(update_rule, &connection, Some(16))
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
        .filter_map(ready))
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
