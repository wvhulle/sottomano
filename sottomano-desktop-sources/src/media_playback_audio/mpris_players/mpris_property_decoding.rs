use std::{fmt::Display, str::FromStr, time::Instant};

use tokio::time::Duration;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

use super::{PlayerSetting, Progress, QueueOrder, SeekMode};
use crate::system_protocol_clients::dbus_desktop_buses::{Properties, prop, text_prop};

pub(super) const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

impl SeekMode {
    pub(super) fn declared(metadata: &Properties) -> Self {
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

impl Progress {
    pub(super) fn declared(player: &Properties, metadata: &Properties) -> Self {
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
}

pub(super) fn controllable_setting<Setting>(
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

pub(super) fn declared_queue_order(player: &Properties) -> Option<QueueOrder> {
    let shuffled = prop::<bool>(player, "Shuffle")?;
    Some(if shuffled {
        QueueOrder::Shuffled
    } else {
        QueueOrder::InOrder
    })
}

pub(super) fn artists(metadata: &Properties) -> Vec<String> {
    prop::<Vec<String>>(metadata, "xesam:artist")
        .unwrap_or_default()
        .into_iter()
        .filter(|artist| !artist.trim().is_empty())
        .collect()
}

pub(super) fn parsed<Parsed: FromStr>(metadata: &Properties, key: &str) -> Option<Parsed>
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
