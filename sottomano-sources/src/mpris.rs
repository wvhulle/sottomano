use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
    time::Instant,
};

use futures::{Stream, StreamExt, future::ready, stream::select};
use image::{DynamicImage, RgbaImage};
use reqwest::Url;
use tokio::time::Duration;
use uom::si::{f64::Ratio, ratio::ratio};
use zbus::{
    Connection, MatchRule, MessageStream,
    blocking::Connection as BlockingConnection,
    fdo::{DBusProxy, NameOwnerChanged, PropertiesProxy},
    message::Type,
    names::{BusName, InterfaceName},
    proxy,
    proxy::CacheProperties,
    zvariant::{ObjectPath, OwnedObjectPath},
};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    background_work::blocking,
    bus::{DbusPropertyLookup, DesktopBuses, OwnerProcess, Properties},
    stream_operators::{connected_until_failure, read_once_then_per_burst},
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYERCTLD: &str = "org.mpris.MediaPlayer2.playerctld";
const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT_INTERFACE: &str = "org.mpris.MediaPlayer2";
const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

impl FromStr for Status {
    type Err = zbus::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Playing" => Ok(Self::Playing),
            "Paused" => Ok(Self::Paused),
            "Stopped" => Ok(Self::Stopped),
            unknown => Err(zbus::Error::Failure(format!(
                "unknown PlaybackStatus {unknown}"
            ))),
        }
    }
}

impl Status {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Off,
    Track,
    Playlist,
}

impl FromStr for LoopMode {
    type Err = zbus::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "None" => Ok(Self::Off),
            "Track" => Ok(Self::Track),
            "Playlist" => Ok(Self::Playlist),
            unknown => Err(zbus::Error::Failure(format!(
                "unknown LoopStatus {unknown}"
            ))),
        }
    }
}

impl Display for LoopMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Off => "None",
            Self::Track => "Track",
            Self::Playlist => "Playlist",
        })
    }
}

impl LoopMode {
    #[must_use]
    pub const fn cycled(self) -> Self {
        match self {
            Self::Off => Self::Playlist,
            Self::Playlist => Self::Track,
            Self::Track => Self::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeekMode {
    Absolute(OwnedObjectPath),
    Relative,
}

impl SeekMode {
    fn declared(metadata: &Properties) -> Self {
        let Some(track) = metadata.object_path("mpris:trackid") else {
            return Self::Relative;
        };
        if track.as_str() == NO_TRACK {
            return Self::Relative;
        }
        Self::Absolute(track)
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub play: bool,
    pub pause: bool,
    pub stop: bool,
    pub seek: bool,
    pub go_next: bool,
    pub go_previous: bool,
    pub raise: bool,
    pub quit: bool,
}

impl Capabilities {
    fn declared(player: &Properties, root: &Properties) -> Self {
        let control = player.flag("CanControl");
        let controllable = |key| control && player.flag(key);
        Self {
            play: controllable("CanPlay"),
            pause: controllable("CanPause"),
            stop: control,
            seek: controllable("CanSeek"),
            go_next: controllable("CanGoNext"),
            go_previous: controllable("CanGoPrevious"),
            raise: root.flag("CanRaise"),
            quit: root.flag("CanQuit"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    SetPosition(OwnedObjectPath, Duration),
    Seek(i64),
    SetShuffle(bool),
    SetLoopMode(LoopMode),
    Raise,
    Quit,
}

#[derive(Debug, Clone, Copy)]
pub enum Progress {
    Untimed,
    Timed {
        position: Duration,
        sampled_at: Instant,
        length: Duration,
        rate: f64,
    },
}

impl Progress {
    fn declared(player: &Properties, metadata: &Properties) -> Self {
        Self::timed(player, metadata).unwrap_or(Self::Untimed)
    }

    fn timed(player: &Properties, metadata: &Properties) -> Option<Self> {
        let length = length(metadata).filter(|length| !length.is_zero())?;
        let position = u64::try_from(player.integer("Position")?).ok()?;
        let rate = player.float("Rate")?;
        Some(Self::Timed {
            position: Duration::from_micros(position),
            sampled_at: Instant::now(),
            length,
            rate,
        })
    }

    #[must_use]
    pub fn fraction(self, status: Status, now: Instant) -> Option<Ratio> {
        let Self::Timed { length, .. } = self else {
            return None;
        };
        let played = self.played(status, now)?;
        Some(Ratio::new::<ratio>(
            played.as_secs_f64() / length.as_secs_f64(),
        ))
    }

    #[must_use]
    pub fn remaining(self, status: Status, now: Instant) -> Option<Duration> {
        let Self::Timed { length, .. } = self else {
            return None;
        };
        Some(length.saturating_sub(self.played(status, now)?))
    }

    #[must_use]
    pub fn length(self) -> Option<Duration> {
        match self {
            Self::Untimed => None,
            Self::Timed { length, .. } => Some(length),
        }
    }

    #[must_use]
    pub fn played(self, status: Status, now: Instant) -> Option<Duration> {
        let Self::Timed {
            position,
            sampled_at,
            length,
            rate,
        } = self
        else {
            return None;
        };
        let elapsed = match status {
            Status::Playing => now
                .saturating_duration_since(sampled_at)
                .mul_f64(rate.max(0.0)),
            Status::Paused | Status::Stopped => Duration::ZERO,
        };
        Some((position + elapsed).min(length))
    }
}

#[derive(Debug, Clone)]
pub struct Media {
    pub bus: String,
    pub owner: OwnerProcess,
    pub identity: String,
    pub desktop_entry: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub url: Option<Url>,
    pub status: Status,
    pub progress: Progress,
    pub art_url: Option<String>,
    pub capabilities: Capabilities,
    pub shuffle: Option<bool>,
    pub loop_mode: Option<LoopMode>,
    pub seek_mode: SeekMode,
}

fn controllable_setting<T>(player: &Properties, declared: Option<T>) -> Option<T> {
    declared.filter(|_| player.flag("CanControl"))
}

fn artist(metadata: &Properties) -> Option<String> {
    metadata
        .get("xesam:artist")
        .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
        .map(|artists| artists.join(", "))
        .filter(|artists| !artists.is_empty())
}

fn parsed<T: FromStr>(metadata: &Properties, key: &str) -> Option<T> {
    metadata.text(key)?.parse().ok()
}

fn length(metadata: &Properties) -> Option<Duration> {
    let value = metadata.get("mpris:length")?;
    let micros = u64::try_from(value).ok().or_else(|| {
        i64::try_from(value)
            .ok()
            .and_then(|v| u64::try_from(v).ok())
    })?;
    Some(Duration::from_micros(micros))
}

#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    fn play(&self) -> zbus::Result<()>;
    fn pause(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    fn seek(&self, offset: i64) -> zbus::Result<()>;
    fn set_position(&self, track_id: &ObjectPath<'_>, position: i64) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_shuffle(&self, value: bool) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_loop_status(&self, value: &str) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.mpris.MediaPlayer2",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait MediaPlayer2 {
    fn raise(&self) -> zbus::Result<()>;
    fn quit(&self) -> zbus::Result<()>;
}

async fn properties(
    connection: &Connection,
    bus: &str,
    interface: &'static str,
) -> zbus::Result<Properties> {
    let proxy = PropertiesProxy::builder(connection)
        .destination(bus.to_owned())?
        .path(PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    Ok(proxy
        .get_all(InterfaceName::from_static_str_unchecked(interface))
        .await?)
}

async fn owner_process(dbus: &DBusProxy<'_>, bus: &str) -> OwnerProcess {
    match BusName::try_from(bus) {
        Ok(name) => OwnerProcess::of_bus(dbus, name).await,
        Err(_) => OwnerProcess::Unidentified,
    }
}

async fn read_media(
    connection: &Connection,
    dbus: &DBusProxy<'_>,
    bus: &str,
) -> zbus::Result<Media> {
    let root = properties(connection, bus, ROOT_INTERFACE).await?;
    let player = properties(connection, bus, PLAYER_INTERFACE).await?;
    let Some(status) = player.text("PlaybackStatus") else {
        return Err(zbus::Error::Failure(
            "PlaybackStatus is not declared".to_owned(),
        ));
    };
    let status = status.parse()?;
    let metadata = player.dictionary("Metadata").unwrap_or_default();
    Ok(Media {
        bus: bus.to_owned(),
        owner: owner_process(dbus, bus).await,
        identity: root.text("Identity").unwrap_or_else(|| bus.to_owned()),
        desktop_entry: root.text("DesktopEntry"),
        title: metadata.text("xesam:title"),
        artist: artist(&metadata),
        album: metadata.text("xesam:album"),
        url: parsed(&metadata, "xesam:url"),
        status,
        progress: Progress::declared(&player, &metadata),
        art_url: metadata.text("mpris:artUrl"),
        capabilities: Capabilities::declared(&player, &root),
        shuffle: controllable_setting(&player, player.boolean("Shuffle")),
        loop_mode: controllable_setting(&player, parsed(&player, "LoopStatus")),
        seek_mode: SeekMode::declared(&metadata),
    })
}

async fn mpris_names(dbus: &DBusProxy<'_>) -> zbus::Result<Vec<String>> {
    Ok(dbus
        .list_names()
        .await?
        .into_iter()
        .map(|name| name.to_string())
        .filter(|name| name.starts_with(PREFIX) && name != PLAYERCTLD)
        .collect())
}

async fn read_all(connection: &Connection, dbus: &DBusProxy<'_>) -> zbus::Result<Vec<Media>> {
    let mut players = Vec::new();
    for bus in mpris_names(dbus).await? {
        match read_media(connection, dbus, &bus).await {
            Ok(media) => players.push(media),
            Err(e) => tracing::debug!("mpris {bus}: {e}"),
        }
    }
    Ok(players)
}

pub async fn load_art(url: &str) -> Option<RgbaImage> {
    if let Some(path) = url.strip_prefix("file://") {
        let path = path.to_owned();
        return blocking("cover art file", move || {
            image::open(path).ok().map(DynamicImage::into_rgba8)
        })
        .await;
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
        return blocking("cover art download", move || {
            image::load_from_memory(&bytes)
                .ok()
                .map(DynamicImage::into_rgba8)
        })
        .await;
    }
    None
}

impl ReadingSource for Vec<Media> {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "mpris";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        connected_until_failure(connect(buses.session)).map(SourceDelivery::Reading)
    }
}

async fn connect(
    connection: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<Media>>>> {
    let dbus = DBusProxy::new(&connection).await?;
    let properties = signals(
        &connection,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
    )
    .await?
    .map(drop);
    let seeks = signals(&connection, PLAYER_INTERFACE, "Seeked")
        .await?
        .map(drop);
    let owners = dbus
        .receive_name_owner_changed()
        .await?
        .filter(|signal| ready(names_player(signal)))
        .map(drop);
    Ok(read_once_then_per_burst(
        select(select(properties, seeks), owners),
        move || {
            let connection = connection.clone();
            let dbus = dbus.clone();
            async move { read_all(&connection, &dbus).await }
        },
    ))
}

async fn signals(
    connection: &Connection,
    interface: &'static str,
    member: &'static str,
) -> zbus::Result<MessageStream> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .path(PATH)?
        .interface(interface)?
        .member(member)?
        .build();
    MessageStream::for_match_rule(rule, connection, Some(16)).await
}

fn names_player(signal: &NameOwnerChanged) -> bool {
    signal
        .args()
        .is_ok_and(|args| args.name().as_str().starts_with(PREFIX))
}

#[must_use]
pub fn control(bus: &str, command: Command) -> bool {
    send_command(bus, command)
        .inspect_err(|e| tracing::warn!("mpris {bus}: {e}"))
        .is_ok()
}

fn send_command(bus: &str, command: Command) -> zbus::Result<()> {
    let connection = BlockingConnection::session()?;
    let player = || {
        PlayerProxyBlocking::builder(&connection)
            .destination(bus.to_owned())?
            .build()
    };
    let root = || {
        MediaPlayer2ProxyBlocking::builder(&connection)
            .destination(bus.to_owned())?
            .build()
    };
    match command {
        Command::Play => player()?.play(),
        Command::Pause => player()?.pause(),
        Command::Stop => player()?.stop(),
        Command::Next => player()?.next(),
        Command::Previous => player()?.previous(),
        Command::SetPosition(track, position) => player()?.set_position(&track, micros(position)),
        Command::Seek(offset) => player()?.seek(offset),
        Command::SetShuffle(shuffle) => player()?.set_shuffle(shuffle),
        Command::SetLoopMode(mode) => player()?.set_loop_status(&mode.to_string()),
        Command::Raise => root()?.raise(),
        Command::Quit => root()?.quit(),
    }
}

#[must_use]
pub fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::{ObjectPath, OwnedValue, Value};

    use super::*;

    fn owned<'a>(value: impl Into<Value<'a>>) -> OwnedValue {
        OwnedValue::try_from(value.into()).unwrap()
    }

    fn all_player_capabilities(control: bool) -> Properties {
        Properties::from([
            ("CanControl".to_owned(), owned(control)),
            ("CanPlay".to_owned(), owned(true)),
            ("CanPause".to_owned(), owned(true)),
            ("CanSeek".to_owned(), owned(true)),
            ("CanGoNext".to_owned(), owned(true)),
            ("CanGoPrevious".to_owned(), owned(true)),
            ("Shuffle".to_owned(), owned(true)),
            ("LoopStatus".to_owned(), owned("Track")),
        ])
    }

    #[test]
    fn uncontrollable_player_declares_no_player_capabilities() {
        let player = all_player_capabilities(false);
        let root = Properties::from([
            ("CanRaise".to_owned(), owned(true)),
            ("CanQuit".to_owned(), owned(true)),
        ]);
        assert_eq!(
            Capabilities::declared(&player, &root),
            Capabilities {
                raise: true,
                quit: true,
                ..Capabilities::default()
            }
        );
        assert_eq!(
            controllable_setting(&player, player.boolean("Shuffle")),
            None
        );
    }

    #[test]
    fn controllable_player_exposes_declared_settings() {
        let player = all_player_capabilities(true);
        assert_eq!(
            controllable_setting(&player, parsed(&player, "LoopStatus")),
            Some(LoopMode::Track)
        );
        assert_eq!(
            controllable_setting(&player, player.boolean("Shuffle")),
            Some(true)
        );
    }

    #[test]
    fn missing_can_pause_is_false() {
        let player = Properties::from([
            ("CanControl".to_owned(), owned(true)),
            ("CanPlay".to_owned(), owned(true)),
        ]);
        let capabilities = Capabilities::declared(&player, &Properties::new());
        assert!(capabilities.play);
        assert!(!capabilities.pause);
    }

    #[test]
    fn loop_mode_cycles_through_every_mode() {
        assert_eq!(LoopMode::Off.cycled(), LoopMode::Playlist);
        assert_eq!(LoopMode::Playlist.cycled(), LoopMode::Track);
        assert_eq!(LoopMode::Track.cycled(), LoopMode::Off);
    }

    #[test]
    fn loop_mode_round_trips_through_its_spec_name() {
        [LoopMode::Off, LoopMode::Playlist, LoopMode::Track]
            .into_iter()
            .for_each(|mode| assert_eq!(mode.to_string().parse::<LoopMode>().ok(), Some(mode)));
        assert!("Shuffled".parse::<LoopMode>().is_err());
    }

    #[test]
    fn seek_mode_is_relative_without_a_real_track() {
        assert_eq!(SeekMode::declared(&Properties::new()), SeekMode::Relative);
        let no_track = Properties::from([(
            "mpris:trackid".to_owned(),
            owned(ObjectPath::try_from(NO_TRACK).unwrap()),
        )]);
        assert_eq!(SeekMode::declared(&no_track), SeekMode::Relative);
        let track = ObjectPath::try_from("/org/mpd/Tracks/7").unwrap();
        let declared = Properties::from([("mpris:trackid".to_owned(), owned(track.clone()))]);
        assert_eq!(
            SeekMode::declared(&declared),
            SeekMode::Absolute(track.into())
        );
    }

    #[test]
    fn played_time_advances_at_the_declared_rate() {
        let sampled_at = Instant::now();
        let progress = Progress::Timed {
            position: Duration::from_secs(10),
            sampled_at,
            length: Duration::from_secs(100),
            rate: 2.0,
        };
        let later = sampled_at + Duration::from_secs(5);
        assert_eq!(
            progress.played(Status::Playing, later),
            Some(Duration::from_secs(20))
        );
        assert_eq!(
            progress.played(Status::Paused, later),
            Some(Duration::from_secs(10))
        );
    }
}
