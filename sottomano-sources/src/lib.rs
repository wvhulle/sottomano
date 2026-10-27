pub mod ai_usage_spend;
pub mod battery_power_profiles;
pub mod blocking_background_work;
pub mod caldav_tasks_events;
pub mod cpu_memory_compute;
pub mod delivery_cadence_drivers;
pub mod desktop_notifications;
pub mod desktop_windows_apps;
pub mod disk_storage;
pub mod gamepad_controller_input;
pub mod host_machine_identity;
pub mod login_session_presence;
pub mod media_playback_audio;
pub mod network_connectivity;
pub mod non_empty_text;
pub mod screen_brightness_color;
pub mod secret_file_reads;
pub mod sqlite_state_store;
pub mod stream_operators;
pub mod system_protocol_clients;
pub mod systemd_units_journal;
pub mod voice_dictation_input;
pub mod wallpaper_place_article;
pub mod weather;

use std::{
    fmt::{Debug, Display},
    hash::Hash,
};

use futures::Stream;

use crate::delivery_cadence_drivers::StampedCadence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCaption {
    Uncaptioned,
    Captioned(&'static str),
}

pub trait SnapshotSource: Clone + Debug + Sized + Send + 'static {
    type Error: Clone + Debug + Display + Send + 'static;
    type StreamInputs: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
    const CAPTION: SourceCaption = SourceCaption::Uncaptioned;

    fn delivery_stream(
        key: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send;
}

#[derive(Debug, Clone)]
pub enum SnapshotDelivery<S: SnapshotSource> {
    SourceUnavailable,
    SnapshotDelivered {
        snapshot_fetch: Result<S, S::Error>,
        stamped_cadence: StampedCadence,
    },
    SnapshotUnchanged,
}

impl<S: SnapshotSource + PartialEq> PartialEq for SnapshotDelivery<S> {
    fn eq(&self, other: &Self) -> bool {
        match self {
            SnapshotDelivery::SourceUnavailable => {
                matches!(other, SnapshotDelivery::SourceUnavailable)
            }
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(snapshot),
                stamped_cadence,
            } => matches!(
                other,
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(known_snapshot),
                    stamped_cadence: known_cadence,
                } if known_snapshot == snapshot && known_cadence == stamped_cadence
            ),
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Err(_),
                ..
            } => false,
            SnapshotDelivery::SnapshotUnchanged => {
                matches!(other, SnapshotDelivery::SnapshotUnchanged)
            }
        }
    }
}
