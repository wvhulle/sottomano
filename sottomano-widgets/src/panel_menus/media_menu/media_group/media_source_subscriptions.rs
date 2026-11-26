use std::{iter, time::Duration};

use iced::{Subscription, time::every};
use sottomano_desktop_sources::{
    media_playback_audio::{
        lrclib_synced_lyrics::{LyricsQuery, TrackLyrics},
        mpris_players::{Media, PlayerBusOwners, Progress, Status},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::indicator_groups::media_lyrics::{LrclibLookup, MediaLyricsConfig};

use super::{MediaGroup, MediaMessage};
use crate::{
    background_work::snapshot_subscription, group_menus::group_source_demand::GroupSourceDemand,
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const PROGRESS_TICK: Duration = Duration::from_secs(1);
const LYRICS_TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlaybackPace {
    Singing,
    Progressing,
    Still,
}

impl MediaGroup {
    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        lyrics_config: &MediaLyricsConfig,
        demand: &GroupSourceDemand,
    ) -> Subscription<MediaMessage> {
        let owner_samples =
            snapshot_subscription::<PlayerBusOwners>(buses.clone()).map(MediaMessage::PlayerOwners);
        let player_samples: Vec<Subscription<MediaMessage>> = self
            .listed_player_owners()
            .map(|player| {
                snapshot_subscription::<Media>(OnBus {
                    buses: buses.clone(),
                    inputs: player.clone(),
                })
                .with(player.bus.clone())
                .map(|(bus, delivery)| MediaMessage::PlayerSample {
                    bus,
                    delivery: Box::new(delivery),
                })
            })
            .collect();
        let lyrics_lookups = self
            .unfetched_lyrics_queries()
            .filter(|_| lyrics_config.lrclib_lookup == LrclibLookup::Enabled)
            .map(|query| {
                snapshot_subscription::<TrackLyrics>(OnBus {
                    buses: buses.clone(),
                    inputs: query.clone(),
                })
                .with(query)
                .map(|(query, delivery)| MediaMessage::LyricsIndicator { query, delivery })
            });
        let progress_tick = match self.playback_pace() {
            PlaybackPace::Singing => {
                Some(every(LYRICS_TICK).map(|_| MediaMessage::ProgressAdvanced))
            }
            PlaybackPace::Progressing => {
                Some(every(PROGRESS_TICK).map(|_| MediaMessage::ProgressAdvanced))
            }
            PlaybackPace::Still => None,
        }
        .map(|tick| demand.while_headline_shown(tick));
        Subscription::batch(
            iter::once(owner_samples)
                .chain(player_samples)
                .chain(lyrics_lookups)
                .chain(progress_tick),
        )
    }

    fn playing_lyrics_queries(&self) -> impl Iterator<Item = LyricsQuery> {
        self.all_players()
            .filter(|media| media.status == Status::Playing)
            .filter_map(Media::lyrics_query)
    }

    pub(super) fn unfetched_lyrics_queries(&self) -> impl Iterator<Item = LyricsQuery> {
        self.playing_lyrics_queries().filter(|query| {
            self.lyrics_snapshots
                .get(query)
                .is_none_or(|lyrics_snapshot| lyrics_snapshot.get().is_none())
        })
    }

    pub(super) fn playback_pace(&self) -> PlaybackPace {
        let singing = self.playing_lyrics_queries().any(|query| {
            matches!(
                self.lyrics_snapshots
                    .get(&query)
                    .and_then(SubscribedSnapshot::get),
                Some(TrackLyrics::Synced(_))
            )
        });
        let progressing = self.all_players().any(|media| {
            media.status == Status::Playing && matches!(media.progress, Progress::Timed { .. })
        });
        match (singing, progressing) {
            (true, _) => PlaybackPace::Singing,
            (false, true) => PlaybackPace::Progressing,
            (false, false) => PlaybackPace::Still,
        }
    }
}
