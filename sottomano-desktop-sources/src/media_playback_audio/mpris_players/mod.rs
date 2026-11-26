use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
    time::Instant,
};

use image::{DynamicImage, RgbaImage};
use tokio::time::Duration;
use uom::si::{f64::Ratio, ratio::ratio};
use zbus::{
    proxy,
    zvariant::{ObjectPath, OwnedObjectPath},
};

pub use super::{
    mpris_player_capabilities::{Capabilities, CommandOffer},
    mpris_track_fields::{CoverArtUrl, TrackAlbum, TrackSource, TrackTitle},
};
use crate::{
    blocking_background_work::blocking, media_playback_audio::lrclib_synced_lyrics::LyricsQuery,
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};

mod mpris_player_bus_streams;
mod mpris_property_decoding;

const PREFIX: &str = "org.mpris.MediaPlayer2.";

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

#[must_use]
pub fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros())
        .expect("track positions are far shorter than i64 microseconds")
}

#[cfg(test)]
mod mpris_player_tests;
