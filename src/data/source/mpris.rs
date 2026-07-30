//! Media playback via MPRIS2 over the session bus (odin's `MediaService`),
//! signal-driven: a subscription watches `PropertiesChanged` on the mpris
//! object path plus `NameOwnerChanged`, and re-reads the active player on each
//! — so track/status/art updates land instantly instead of on a poll tick.

use std::collections::HashMap;

use iced::{
    Subscription,
    futures::{
        SinkExt, StreamExt,
        channel::mpsc::Sender,
        stream::{iter, select},
    },
    stream,
    widget::image,
};
use tokio::time::{Duration, sleep};
use zbus::{
    Connection, MatchRule, MessageStream, Proxy, fdo::DBusProxy, message::Type,
    zvariant::OwnedValue,
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYERCTLD: &str = "org.mpris.MediaPlayer2.playerctld";
const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

impl Status {
    fn parse(s: &str) -> Self {
        match s {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Media {
    pub bus: String,
    pub identity: String,
    /// The player's freedesktop desktop-entry name, when it advertises one —
    /// the most authoritative handle for resolving its app glyph and window.
    pub desktop_entry: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: Status,
    /// The current track's album art, decoded in memory. `None` when the player
    /// exposes no art (or it couldn't be fetched); the widget shows a glyph
    /// then.
    pub art: Option<image::Handle>,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_toggle: bool,
}

impl Media {
    pub fn app_id(&self) -> String {
        self.desktop_entry
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| app_handle(&self.bus))
    }

    /// `"Artist — Title"`, or whichever half is present, or the player
    /// identity.
    pub fn label(&self) -> String {
        match (self.artist.is_empty(), self.title.is_empty()) {
            (false, false) => format!("{} — {}", self.artist, self.title),
            (true, false) => self.title.clone(),
            (false, true) => self.artist.clone(),
            (true, true) => self.identity.clone(),
        }
    }
}

fn string(metadata: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = metadata.get(key)?;
    String::try_from(value.try_clone().ok()?)
        .ok()
        .filter(|s| !s.is_empty())
}

fn artist(metadata: &HashMap<String, OwnedValue>) -> String {
    metadata
        .get("xesam:artist")
        .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
        .map(|artists| artists.join(", "))
        .unwrap_or_default()
}

/// The canonical app handle from an mpris bus name: strip the
/// `org.mpris.MediaPlayer2.` prefix and any chromium/Electron `.instanceNNNN`
/// suffix, then take the last dotted segment. E.g.
/// `…MediaPlayer2.chromium.instance2144933` → `chromium`,
/// `…MediaPlayer2.io.github.htkhiem.Euphonica` → `Euphonica`. Ports odin's
/// `MediaService.busHandle`.
pub fn app_handle(bus: &str) -> String {
    let stripped = bus.strip_prefix(PREFIX).unwrap_or(bus);
    let stripped = stripped
        .rsplit_once('.')
        .filter(|(_, seg)| {
            seg.starts_with("instance")
                && seg.len() > "instance".len()
                && seg["instance".len()..].chars().all(|c| c.is_ascii_digit())
        })
        .map_or(stripped, |(head, _)| head);
    stripped
        .rsplit_once('.')
        .map_or(stripped, |(_, seg)| seg)
        .to_string()
}

/// A bus like `…MediaPlayer2.chromium.instance2144933` is a CEF duplicate when
/// it has no desktop entry — filtered out of the picker.
fn is_phantom(bus: &str, desktop_entry: Option<&str>) -> bool {
    desktop_entry.is_none_or(str::is_empty)
        && bus
            .rsplit('.')
            .next()
            .is_some_and(|seg| seg.starts_with("instance") && seg.len() > "instance".len())
}

async fn property<T>(connection: &Connection, bus: &str, interface: &str, name: &str) -> Option<T>
where
    T: TryFrom<OwnedValue>,
    <T as TryFrom<OwnedValue>>::Error: Into<zbus::Error>,
{
    let proxy = Proxy::new(connection, bus, PATH, interface).await.ok()?;
    proxy.get_property(name).await.ok()
}

/// One player's status, identity, and desktop entry (for phantom detection).
async fn read_player(
    connection: &Connection,
    bus: &str,
) -> Option<(Status, String, Option<String>)> {
    let status = property::<String>(connection, bus, PLAYER, "PlaybackStatus").await?;
    let identity = property::<String>(connection, bus, ROOT, "Identity")
        .await
        .unwrap_or_else(|| bus.to_string());
    let desktop = property::<String>(connection, bus, ROOT, "DesktopEntry").await;
    Some((Status::parse(&status), identity, desktop))
}

/// The current mpris player buses (skipping `playerctld`, which mirrors a real
/// player).
async fn mpris_names(dbus: &DBusProxy<'_>) -> Vec<String> {
    dbus.list_names()
        .await
        .into_iter()
        .flatten()
        .map(|name| name.to_string())
        .filter(|name| name.starts_with(PREFIX) && name != PLAYERCTLD)
        .collect()
}

/// Every non-phantom player's full track + transport state — the bar picks
/// which one to surface and lets the user switch between them. `art` memoises
/// each bus's last fetched `(url, handle)` so art is downloaded once per track,
/// not on every play/pause or status signal; buses that vanish are pruned.
async fn read_all(
    connection: &Connection,
    dbus: &DBusProxy<'_>,
    art: &mut HashMap<String, (String, image::Handle)>,
) -> Vec<Media> {
    let players: Vec<(String, Status, String, Option<String>)> = iter(mpris_names(dbus).await)
        .filter_map(|bus| async move {
            let (status, identity, desktop) = read_player(connection, &bus).await?;
            (!is_phantom(&bus, desktop.as_deref())).then_some((bus, status, identity, desktop))
        })
        .collect()
        .await;
    art.retain(|bus, _| players.iter().any(|(b, ..)| b == bus));

    let mut media = Vec::with_capacity(players.len());
    for (bus, status, identity, desktop_entry) in players {
        let metadata =
            property::<HashMap<String, OwnedValue>>(connection, &bus, PLAYER, "Metadata")
                .await
                .unwrap_or_default();
        let handle = if let Some(url) = string(&metadata, "mpris:artUrl") {
            art_handle(&url, &bus, art).await
        } else {
            art.remove(&bus);
            None
        };
        media.push(Media {
            title: string(&metadata, "xesam:title").unwrap_or_default(),
            artist: artist(&metadata),
            album: string(&metadata, "xesam:album").unwrap_or_default(),
            art: handle,
            can_next: property(connection, &bus, PLAYER, "CanGoNext")
                .await
                .unwrap_or(false),
            can_prev: property(connection, &bus, PLAYER, "CanGoPrevious")
                .await
                .unwrap_or(false),
            can_toggle: property(connection, &bus, PLAYER, "CanPause")
                .await
                .unwrap_or(true),
            status,
            identity,
            desktop_entry,
            bus,
        });
    }
    media
}

/// The decoded album-art handle for `bus`'s `url`, reusing `cache[bus]` when
/// the url is unchanged so a fetch happens only when that player's track (and
/// its art) changes. On any fetch failure the bus's cache entry is cleared and
/// `None` returned.
async fn art_handle(
    url: &str,
    bus: &str,
    cache: &mut HashMap<String, (String, image::Handle)>,
) -> Option<image::Handle> {
    if let Some((cached, handle)) = cache.get(bus)
        && cached == url
    {
        return Some(handle.clone());
    }
    let handle = load_art(url).await;
    match handle.clone() {
        Some(h) => {
            cache.insert(bus.to_string(), (url.to_string(), h));
        }
        None => {
            cache.remove(bus);
        }
    }
    handle
}

/// Decode album art from an mpris `mpris:artUrl`: a local `file://` path, or a
/// remote `http(s)://` image fetched into memory. Any other scheme yields
/// `None`.
async fn load_art(url: &str) -> Option<image::Handle> {
    if let Some(path) = url.strip_prefix("file://") {
        return Some(image::Handle::from_path(path));
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let bytes = reqwest::get(url)
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .bytes()
            .await
            .ok()?;
        return Some(image::Handle::from_bytes(bytes.to_vec()));
    }
    None
}

/// The signal-driven media stream: an empty list while no player is present, a
/// fresh [`Media`] per player on every relevant change. Reconnects if the bus
/// drops.
pub fn subscription() -> Subscription<Vec<Media>> {
    Subscription::run(|| {
        stream::channel(16, |mut output: Sender<Vec<Media>>| async move {
            loop {
                if let Err(e) = watch(&mut output).await {
                    eprintln!("mpris: {e}");
                }
                sleep(Duration::from_secs(2)).await;
            }
        })
    })
}

async fn watch(output: &mut Sender<Vec<Media>>) -> zbus::Result<()> {
    let connection = Connection::session().await?;
    let dbus = DBusProxy::new(&connection).await?;
    let mut art = HashMap::new();
    if output
        .send(read_all(&connection, &dbus, &mut art).await)
        .await
        .is_err()
    {
        return Ok(());
    }

    let properties_rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .path(PATH)?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build();
    let owners_rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .build();
    // Merge both signal streams into one of "is this relevant?" — a property
    // change on any player is always relevant; a name change only when it names
    // an mpris player (one opened or closed).
    let properties = MessageStream::for_match_rule(properties_rule, &connection, Some(16))
        .await?
        .map(|_| true);
    let owners = MessageStream::for_match_rule(owners_rule, &connection, Some(16))
        .await?
        .map(|message| is_mpris_owner_change(message.ok().as_ref()));
    let mut events = select(properties, owners);

    while let Some(relevant) = events.next().await {
        if relevant
            && output
                .send(read_all(&connection, &dbus, &mut art).await)
                .await
                .is_err()
        {
            break;
        }
    }
    Ok(())
}

/// Whether a `NameOwnerChanged` names an mpris player (one opened or closed).
fn is_mpris_owner_change(message: Option<&zbus::Message>) -> bool {
    message
        .and_then(|m| m.body().deserialize::<(String, String, String)>().ok())
        .is_some_and(|(name, _, _)| name.starts_with(PREFIX))
}

/// Run a no-argument control method (`Next`, `Previous`, `PlayPause`, `Stop`)
/// on a player. Blocking one-shot, called off-thread.
pub fn control(bus: &str, method: &str) {
    use zbus::blocking::{Connection, Proxy};
    let Ok(connection) = Connection::session() else {
        return;
    };
    if let Ok(proxy) = Proxy::new(&connection, bus, PATH, PLAYER) {
        let _: zbus::Result<()> = proxy.call(method, &());
    }
}
