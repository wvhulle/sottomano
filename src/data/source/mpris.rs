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
    Connection, MatchRule, MessageStream, fdo::DBusProxy, message::Type, proxy,
    proxy::CacheProperties, zvariant::OwnedValue,
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYERCTLD: &str = "org.mpris.MediaPlayer2.playerctld";
const PATH: &str = "/org/mpris/MediaPlayer2";

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
    pub desktop_entry: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: Status,
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

fn is_phantom(bus: &str, desktop_entry: Option<&str>) -> bool {
    desktop_entry.is_none_or(str::is_empty)
        && bus
            .rsplit('.')
            .next()
            .is_some_and(|seg| seg.starts_with("instance") && seg.len() > "instance".len())
}

#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    fn play_pause(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
    #[zbus(property)]
    fn can_go_next(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn can_go_previous(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn can_pause(&self) -> zbus::Result<bool>;
}

#[proxy(
    interface = "org.mpris.MediaPlayer2",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait MediaPlayer2 {
    #[zbus(property)]
    fn identity(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn desktop_entry(&self) -> zbus::Result<String>;
}

struct PlayerState {
    bus: String,
    status: Status,
    identity: String,
    desktop_entry: Option<String>,
}

async fn player_proxy(connection: &Connection, bus: &str) -> Option<PlayerProxy<'static>> {
    PlayerProxy::builder(connection)
        .destination(bus.to_owned())
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()
}

async fn root_proxy(connection: &Connection, bus: &str) -> Option<MediaPlayer2Proxy<'static>> {
    MediaPlayer2Proxy::builder(connection)
        .destination(bus.to_owned())
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()
}

async fn read_player(connection: &Connection, bus: String) -> Option<PlayerState> {
    let status = Status::parse(
        &player_proxy(connection, &bus)
            .await?
            .playback_status()
            .await
            .ok()?,
    );
    let root = root_proxy(connection, &bus).await?;
    let identity = root.identity().await.unwrap_or_else(|_| bus.clone());
    let desktop_entry = root.desktop_entry().await.ok();
    Some(PlayerState {
        bus,
        status,
        identity,
        desktop_entry,
    })
}

async fn mpris_names(dbus: &DBusProxy<'_>) -> Vec<String> {
    dbus.list_names()
        .await
        .into_iter()
        .flatten()
        .map(|name| name.to_string())
        .filter(|name| name.starts_with(PREFIX) && name != PLAYERCTLD)
        .collect()
}

async fn read_all(
    connection: &Connection,
    dbus: &DBusProxy<'_>,
    art: &mut HashMap<String, (String, image::Handle)>,
) -> Vec<Media> {
    let players: Vec<PlayerState> = iter(mpris_names(dbus).await)
        .filter_map(|bus| async move {
            let state = read_player(connection, bus).await?;
            (!is_phantom(&state.bus, state.desktop_entry.as_deref())).then_some(state)
        })
        .collect()
        .await;
    art.retain(|bus, _| players.iter().any(|state| &state.bus == bus));

    let mut media = Vec::with_capacity(players.len());
    for state in players {
        let player = player_proxy(connection, &state.bus).await;
        let metadata = match &player {
            Some(player) => player.metadata().await.unwrap_or_default(),
            None => HashMap::new(),
        };
        let handle = if let Some(url) = string(&metadata, "mpris:artUrl") {
            art_handle(&url, &state.bus, art).await
        } else {
            art.remove(&state.bus);
            None
        };
        let can_next = match &player {
            Some(player) => player.can_go_next().await.unwrap_or(false),
            None => false,
        };
        let can_prev = match &player {
            Some(player) => player.can_go_previous().await.unwrap_or(false),
            None => false,
        };
        let can_toggle = match &player {
            Some(player) => player.can_pause().await.unwrap_or(true),
            None => true,
        };
        media.push(Media {
            title: string(&metadata, "xesam:title").unwrap_or_default(),
            artist: artist(&metadata),
            album: string(&metadata, "xesam:album").unwrap_or_default(),
            art: handle,
            can_next,
            can_prev,
            can_toggle,
            status: state.status,
            identity: state.identity,
            desktop_entry: state.desktop_entry,
            bus: state.bus,
        });
    }
    media
}

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
    let properties = MessageStream::for_match_rule(properties_rule, &connection, Some(16))
        .await?
        .map(|_| true);
    let owners = dbus.receive_name_owner_changed().await?.map(|signal| {
        signal
            .args()
            .is_ok_and(|args| args.name().as_str().starts_with(PREFIX))
    });
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

pub fn control(bus: &str, method: &str) {
    use zbus::blocking::Connection;
    let Ok(connection) = Connection::session() else {
        return;
    };
    let Ok(proxy) = PlayerProxyBlocking::builder(&connection)
        .destination(bus.to_owned())
        .and_then(zbus::blocking::proxy::Builder::build)
    else {
        return;
    };
    let _ = match method {
        "Next" => proxy.next(),
        "Previous" => proxy.previous(),
        "PlayPause" => proxy.play_pause(),
        "Stop" => proxy.stop(),
        _ => Ok(()),
    };
}
