use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
    time::Instant,
};

use futures::{Stream, StreamExt, future::ready, stream::select};
use image::{DynamicImage, RgbaImage};
use tokio::time::Duration;
use uom::si::{f64::Ratio, ratio::ratio};
use zbus::{
    Connection, MatchRule, MessageStream,
    fdo::{DBusProxy, PropertiesProxy},
    message::Type,
    names::{BusName, InterfaceName},
    proxy,
    proxy::CacheProperties,
    zvariant::{ObjectPath, OwnedObjectPath, Value},
};

pub use super::{
    mpris_player_capabilities::{Capabilities, CommandOffer},
    mpris_track_fields::{CoverArtUrl, TrackAlbum, TrackSource, TrackTitle},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking,
    delivery_cadence_drivers::ChangeTriggerStreams,
    media_playback_audio::lrclib_synced_lyrics::LyricsQuery,
    system_protocol_clients::{
        dbus_change_signals::signal_messages,
        dbus_desktop_buses::{DesktopBuses, OnBus, OwnerProcess, Properties, prop, text_prop},
        protocol_peer_reconnects::subscribed,
    },
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

    fn from_str(status_text: &str) -> Result<Self, Self::Err> {
        match status_text {
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
pub enum QueueOrder {
    InOrder,
    Shuffled,
}

impl QueueOrder {
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            QueueOrder::InOrder => QueueOrder::Shuffled,
            QueueOrder::Shuffled => QueueOrder::InOrder,
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

    fn from_str(status_text: &str) -> Result<Self, Self::Err> {
        match status_text {
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
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
        let Some(track) = declared_track_path(metadata) else {
            return Self::Relative;
        };
        if track.as_str() == NO_TRACK {
            return Self::Relative;
        }
        Self::Absolute(track)
    }
}

fn declared_track_path(metadata: &Properties) -> Option<OwnedObjectPath> {
    match &**metadata.get("mpris:trackid")? {
        Value::ObjectPath(track_path) => Some(track_path.clone().into()),
        Value::Str(track_text) => ObjectPath::try_from(track_text.as_str())
            .map(OwnedObjectPath::from)
            .inspect_err(|path_failure| {
                tracing::warn!(
                    "mpris: mpris:trackid {track_text} is not an object path: {path_failure}"
                );
            })
            .ok(),
        other_value => {
            tracing::warn!(
                "mpris: mpris:trackid has an unexpected type {}",
                other_value.value_signature()
            );
            None
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
    SetQueueOrder(QueueOrder),
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
        let signed_position = prop::<i64>(player, "Position")?;
        let position = u64::try_from(signed_position)
            .inspect_err(|_| {
                tracing::warn!("mpris: player reports negative position {signed_position}");
            })
            .ok()?;
        let rate = prop::<f64>(player, "Rate")?;
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
        Some(
            length
                .checked_sub(self.played(status, now)?)
                .expect("played time is capped at the track length"),
        )
    }

    #[must_use]
    pub fn length(self) -> Option<Duration> {
        match self {
            Self::Untimed => None,
            Self::Timed { length, .. } => Some(length),
        }
    }

    #[must_use]
    #[expect(
        clippy::disallowed_methods,
        reason = "the position is sampled on the source task, which can run after the viewer took \
                  its clock reading"
    )]
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
    pub title: TrackTitle,
    pub artists: Vec<String>,
    pub album: TrackAlbum,
    pub url: TrackSource,
    pub status: Status,
    pub progress: Progress,
    pub art_url: CoverArtUrl,
    pub capabilities: Capabilities,
    pub queue_order: PlayerSetting<QueueOrder>,
    pub loop_mode: PlayerSetting<LoopMode>,
    pub seek_mode: SeekMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSetting<Setting> {
    Controllable(Setting),
    Uncontrollable,
}

fn controllable_setting<Setting>(
    player: &Properties,
    declared: Option<Setting>,
) -> PlayerSetting<Setting> {
    match declared {
        Some(setting) if prop(player, "CanControl").unwrap_or(false) => {
            PlayerSetting::Controllable(setting)
        }
        Some(_) | None => PlayerSetting::Uncontrollable,
    }
}

fn declared_queue_order(player: &Properties) -> Option<QueueOrder> {
    let shuffled = prop::<bool>(player, "Shuffle")?;
    Some(if shuffled {
        QueueOrder::Shuffled
    } else {
        QueueOrder::InOrder
    })
}

impl Media {
    #[must_use]
    pub fn joined_artists(&self) -> Option<String> {
        (!self.artists.is_empty()).then(|| self.artists.join(", "))
    }

    #[must_use]
    pub fn lyrics_query(&self) -> Option<LyricsQuery> {
        let TrackTitle::Titled(title) = &self.title else {
            return None;
        };
        Some(LyricsQuery {
            artist: self.artists.first()?.clone(),
            title: title.clone(),
            album: match &self.album {
                TrackAlbum::OnAlbum(album) => album.clone(),
                TrackAlbum::NoAlbum => String::new(),
            },
            duration_secs: self.progress.length()?.as_secs(),
        })
    }
}

fn artists(metadata: &Properties) -> Vec<String> {
    prop::<Vec<String>>(metadata, "xesam:artist")
        .unwrap_or_default()
        .into_iter()
        .filter(|artist| !artist.trim().is_empty())
        .collect()
}

fn parsed<Parsed: FromStr>(metadata: &Properties, key: &str) -> Option<Parsed>
where
    Parsed::Err: Display,
{
    let raw_text = text_prop(metadata, key)?;
    raw_text
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!("mpris: property {key} has invalid value {raw_text:?}: {parse_failure}");
        })
        .ok()
}

fn length(metadata: &Properties) -> Option<Duration> {
    let value = metadata.get("mpris:length")?;
    if let Ok(micros) = u64::try_from(value) {
        return Some(Duration::from_micros(micros));
    }
    let signed_micros = i64::try_from(value)
        .inspect_err(|type_mismatch| {
            tracing::warn!("mpris: mpris:length has an unexpected type: {type_mismatch}");
        })
        .ok()?;
    let micros = u64::try_from(signed_micros)
        .inspect_err(|_| {
            tracing::warn!("mpris: mpris:length is negative: {signed_micros}");
        })
        .ok()?;
    Some(Duration::from_micros(micros))
}

#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2",
    gen_blocking = false
)]
pub trait Player {
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
    default_path = "/org/mpris/MediaPlayer2",
    gen_blocking = false
)]
pub trait MediaPlayer2 {
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
        Ok(name) => OwnerProcess::query_bus_owner(dbus, name).await,
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
    let Some(status) = text_prop(&player, "PlaybackStatus") else {
        return Err(zbus::Error::Failure(
            "PlaybackStatus is not declared".to_owned(),
        ));
    };
    let status = status.parse()?;
    let metadata = prop::<Properties>(&player, "Metadata").unwrap_or_default();
    Ok(Media {
        bus: bus.to_owned(),
        owner: owner_process(dbus, bus).await,
        identity: text_prop(&root, "Identity").unwrap_or_else(|| bus.to_owned()),
        title: text_prop(&metadata, "xesam:title").map_or(TrackTitle::Untitled, TrackTitle::Titled),
        artists: artists(&metadata),
        album: text_prop(&metadata, "xesam:album").map_or(TrackAlbum::NoAlbum, TrackAlbum::OnAlbum),
        url: parsed(&metadata, "xesam:url").map_or(TrackSource::Unlocated, TrackSource::Located),
        status,
        progress: Progress::declared(&player, &metadata),
        art_url: text_prop(&metadata, "mpris:artUrl")
            .map_or(CoverArtUrl::NotAdvertised, CoverArtUrl::Advertised),
        capabilities: Capabilities::declared(&player, &root),
        queue_order: controllable_setting(&player, declared_queue_order(&player)),
        loop_mode: controllable_setting(&player, parsed(&player, "LoopStatus")),
        seek_mode: SeekMode::declared(&metadata),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerBusOwner {
    pub bus: String,
    pub owner: String,
}

#[must_use]
pub fn mpris_player_name(bus: &str) -> &str {
    bus.strip_prefix(PREFIX)
        .expect("player buses are listed by their MPRIS prefix")
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerBusOwners {
    pub listed_players: Vec<PlayerBusOwner>,
}

async fn listed_player_owners(dbus: &DBusProxy<'_>) -> zbus::Result<PlayerBusOwners> {
    let mut player_owners = Vec::new();
    for name in dbus.list_names().await? {
        let bus = name.to_string();
        if !bus.starts_with(PREFIX) || bus == PLAYERCTLD {
            continue;
        }
        match dbus.get_name_owner(name.into()).await {
            Ok(owner) => player_owners.push(PlayerBusOwner {
                bus,
                owner: owner.to_string(),
            }),
            Err(owner_lookup_failure) => {
                tracing::debug!(
                    "mpris {bus}: left before its owner was looked up: {owner_lookup_failure}"
                );
            }
        }
    }
    player_owners.sort();
    Ok(PlayerBusOwners {
        listed_players: player_owners,
    })
}

pub async fn load_art(url: &str) -> Option<RgbaImage> {
    if let Some(path) = url.strip_prefix("file://") {
        let path = path.to_owned();
        return blocking("cover art file", move || {
            image::open(&path)
                .inspect_err(|open_failure| {
                    tracing::warn!("mpris: cannot open cover art {path}: {open_failure}");
                })
                .ok()
                .map(DynamicImage::into_rgba8)
        })
        .await;
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let download = async { reqwest::get(url).await?.error_for_status()?.bytes().await };
        let bytes = download
            .await
            .inspect_err(|download_failure| {
                tracing::warn!("mpris: cannot download cover art {url}: {download_failure}");
            })
            .ok()?;
        let art_url = url.to_owned();
        return blocking("cover art download", move || {
            image::load_from_memory(&bytes)
                .inspect_err(|decode_failure| {
                    tracing::warn!("mpris: cannot decode cover art {art_url}: {decode_failure}");
                })
                .ok()
                .map(DynamicImage::into_rgba8)
        })
        .await;
    }
    None
}

impl SnapshotSource for PlayerBusOwners {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Media players";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        subscribed(buses.session, connect_player_owners).map(SnapshotDelivery::live_push)
    }
}

async fn connect_player_owners(
    connection: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<PlayerBusOwners>>> {
    let dbus = DBusProxy::new(&connection).await?;
    let ownership_changes = dbus
        .receive_name_owner_changed()
        .await?
        .filter(|signal| {
            ready(
                signal
                    .args()
                    .is_ok_and(|args| args.name().as_str().starts_with(PREFIX)),
            )
        })
        .map(|_| Ok::<(), zbus::Error>(()));
    Ok(ownership_changes.triggered_rereads(move || {
        let dbus = dbus.clone();
        async move { listed_player_owners(&dbus).await }
    }))
}

impl SnapshotSource for Media {
    type Error = zbus::Error;
    type StreamInputs = OnBus<PlayerBusOwner>;

    const NAME: &'static str = "Media player";

    fn delivery_stream(
        OnBus {
            buses,
            inputs: player,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        subscribed(buses.session, move |connection| {
            connect_player(connection, player.clone())
        })
        .map(SnapshotDelivery::live_push)
    }
}

async fn connect_player(
    connection: Connection,
    player: PlayerBusOwner,
) -> zbus::Result<impl Stream<Item = zbus::Result<Media>>> {
    let dbus = DBusProxy::new(&connection).await?;
    let properties = player_signals(
        &connection,
        &player.owner,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
    )
    .await?
    .map(|message| message.map(drop));
    let seeks = player_signals(&connection, &player.owner, PLAYER_INTERFACE, "Seeked")
        .await?
        .map(|message| message.map(drop));
    Ok(select(properties, seeks).triggered_rereads(move || {
        let connection = connection.clone();
        let dbus = dbus.clone();
        let bus = player.bus.clone();
        async move { read_media(&connection, &dbus, &bus).await }
    }))
}

async fn player_signals(
    connection: &Connection,
    owner: &str,
    interface: &'static str,
    member: &'static str,
) -> zbus::Result<MessageStream> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(owner)?
        .path(PATH)?
        .interface(interface)?
        .member(member)?
        .build()
        .into_owned();
    signal_messages(connection, rule).await
}

#[must_use]
pub fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros())
        .expect("track positions are far shorter than i64 microseconds")
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::{ObjectPath, OwnedValue, Value};

    use super::*;

    fn owned<'value>(value: impl Into<Value<'value>>) -> OwnedValue {
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
                raise: CommandOffer::Offered,
                quit: CommandOffer::Offered,
                ..Capabilities::default()
            }
        );
        assert_eq!(
            controllable_setting(&player, declared_queue_order(&player)),
            PlayerSetting::Uncontrollable
        );
    }

    #[test]
    fn controllable_player_exposes_declared_settings() {
        let player = all_player_capabilities(true);
        assert_eq!(
            controllable_setting(&player, parsed(&player, "LoopStatus")),
            PlayerSetting::Controllable(LoopMode::Track)
        );
        assert_eq!(
            controllable_setting(&player, declared_queue_order(&player)),
            PlayerSetting::Controllable(QueueOrder::Shuffled)
        );
    }

    #[test]
    fn missing_can_pause_is_false() {
        let player = Properties::from([
            ("CanControl".to_owned(), owned(true)),
            ("CanPlay".to_owned(), owned(true)),
        ]);
        let capabilities = Capabilities::declared(&player, &Properties::new());
        assert_eq!(capabilities.play, CommandOffer::Offered);
        assert_eq!(capabilities.pause, CommandOffer::NotOffered);
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
        let spotify_track = "/com/spotify/track/4Rrhbvf23LWmr4IxwOBGrH";
        let declared_as_text =
            Properties::from([("mpris:trackid".to_owned(), owned(spotify_track))]);
        assert_eq!(
            SeekMode::declared(&declared_as_text),
            SeekMode::Absolute(ObjectPath::try_from(spotify_track).unwrap().into())
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
