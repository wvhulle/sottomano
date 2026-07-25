//! Media playback via MPRIS2 over the session bus (odin's `MediaService`). Polls
//! the connected `org.mpris.MediaPlayer2.*` players, picks the one to control
//! (playing > paused > first, skipping CEF phantoms), and reads its track +
//! transport state. Controls are one-shot method calls.

use std::{collections::HashMap, time::Duration};

use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedValue,
};

use crate::{
    data_sources::{BarWidget, Source, TooltipBody},
    widgets::icon::Icon,
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
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
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: Status,
    pub art_url: Option<String>,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_toggle: bool,
}

impl Media {
    /// `"Artist — Title"`, or whichever half is present, or the player identity.
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

/// A bus like `…MediaPlayer2.chromium.instance2144933` is a CEF duplicate when
/// it has no desktop entry — odin filters these out of the picker.
fn is_phantom(bus: &str, desktop_entry: Option<&str>) -> bool {
    desktop_entry.is_none_or(str::is_empty)
        && bus
            .rsplit('.')
            .next()
            .is_some_and(|seg| seg.starts_with("instance") && seg.len() > "instance".len())
}

fn player_names(connection: &Connection) -> Option<Vec<String>> {
    let proxy = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .ok()?;
    let names: Vec<String> = proxy.call("ListNames", &()).ok()?;
    Some(names.into_iter().filter(|n| n.starts_with(PREFIX)).collect())
}

fn property<T>(connection: &Connection, bus: &str, interface: &str, name: &str) -> Option<T>
where
    T: TryFrom<OwnedValue>,
    <T as TryFrom<OwnedValue>>::Error: Into<zbus::Error>,
{
    let proxy = Proxy::new(connection, bus, PATH, interface).ok()?;
    proxy.get_property(name).ok()
}

impl Media {
    fn read(connection: &Connection, bus: &str) -> Option<(Status, String, Option<String>)> {
        let status = property::<String>(connection, bus, PLAYER, "PlaybackStatus")?;
        let identity =
            property::<String>(connection, bus, ROOT, "Identity").unwrap_or_else(|| bus.to_string());
        let desktop = property::<String>(connection, bus, ROOT, "DesktopEntry");
        Some((Status::parse(&status), identity, desktop))
    }
}

/// Run a no-argument control method (`Next`, `Previous`, `PlayPause`, `Stop`) on
/// a player.
pub fn control(bus: &str, method: &str) {
    let Ok(connection) = Connection::session() else {
        return;
    };
    if let Ok(proxy) = Proxy::new(&connection, bus, PATH, PLAYER) {
        let _: zbus::Result<()> = proxy.call(method, &());
    }
}

// Inert: the media group renders the current player directly, not via the
// shared reading path.
impl BarWidget for Media {
    fn icon(&self) -> Icon {
        Icon::Music
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

impl Source for Media {
    const NAME: &'static str = "mpris";
    const INTERVAL: Duration = Duration::from_secs(1);

    fn poll() -> Option<Self> {
        let connection = Connection::session().ok()?;
        let names = player_names(&connection)?;
        let players: Vec<(String, Status, String)> = names
            .into_iter()
            .filter_map(|bus| {
                let (status, identity, desktop) = Media::read(&connection, &bus)?;
                (!is_phantom(&bus, desktop.as_deref())).then_some((bus, status, identity))
            })
            .collect();
        if players.is_empty() {
            return None;
        }

        // Pick: first Playing, else first Paused, else first.
        let pick = |want: Status| players.iter().position(|(_, s, _)| *s == want);
        let index = pick(Status::Playing)
            .or_else(|| pick(Status::Paused))
            .unwrap_or(0);
        let (bus, status, identity) = players[index].clone();

        let metadata =
            property::<HashMap<String, OwnedValue>>(&connection, &bus, PLAYER, "Metadata")
                .unwrap_or_default();

        Some(Media {
            title: string(&metadata, "xesam:title").unwrap_or_default(),
            artist: artist(&metadata),
            album: string(&metadata, "xesam:album").unwrap_or_default(),
            art_url: string(&metadata, "mpris:artUrl"),
            can_next: property(&connection, &bus, PLAYER, "CanGoNext").unwrap_or(false),
            can_prev: property(&connection, &bus, PLAYER, "CanGoPrevious").unwrap_or(false),
            can_toggle: property(&connection, &bus, PLAYER, "CanPause").unwrap_or(true),
            status,
            identity,
            bus,
        })
    }
}
