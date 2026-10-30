use std::time::Duration;

use reqwest::Url;
use sottomano_sources::{
    SnapshotDelivery,
    media_playback_audio::{
        lrclib_synced_lyrics::TrackLyrics,
        mpris_players::{Capabilities, LoopMode, Media, Progress, SeekMode, Status},
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    widget_groups::media_group::{MediaGroup, MediaMessage},
};

use crate::showcased_group::{ShowcasedGroup, ShowcasedMedia};

pub const SPOTIFY_BUS: &str = "org.mpris.MediaPlayer2.spotify";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = MediaGroup::default();
    let _ = group.update(
        MediaMessage::Reading(SnapshotDelivery::live_push(Ok(vec![
            now_playing(clock),
            paused_video(clock),
        ]))),
        clock,
    );
    if let Some(query) = now_playing(clock).lyrics_query() {
        let _ = group.update(
            MediaMessage::LyricsReading {
                query,
                delivery: SnapshotDelivery::single_read(Ok(low_tide_lyrics())),
            },
            clock,
        );
    }
    ShowcasedGroup::Media(Box::new(ShowcasedMedia {
        group,
        desktop_entries: DesktopIndex::default(),
    }))
}

fn low_tide_lyrics() -> TrackLyrics {
    TrackLyrics::synced_from_lrc(
        "[01:02.00]Salt on the window, sand in the sheets\n[01:08.00]Gulls over the harbour \
         wall\n[01:14.00]You said the sea would keep\n[01:20.00]<01:20.00>Low <01:20.90>tide, \
         <01:22.40>low <01:23.30>tide\n[01:26.00]Everything we lost comes back in\n[01:32.00]Low \
         tide, low tide\n[01:38.00]",
    )
}

fn now_playing(clock: Clock) -> Media {
    Media {
        bus: SPOTIFY_BUS.to_owned(),
        owner: OwnerProcess::Unidentified,
        identity: "Spotify".to_owned(),
        desktop_entry: Some("spotify".to_owned()),
        title: Some("Low Tide".to_owned()),
        artists: vec!["The Harbour Lights".to_owned()],
        album: Some("Saltwater Songs".to_owned()),
        url: Url::parse("https://music.youtube.com/watch?v=low-tide-saltwater-songs").ok(),
        status: Status::Playing,
        progress: Progress::Timed {
            position: Duration::from_secs(83),
            sampled_at: clock.instant,
            length: Duration::from_secs(241),
            rate: 1.0,
        },
        art_url: None,
        capabilities: Capabilities {
            play: true,
            pause: true,
            stop: true,
            seek: true,
            go_next: true,
            go_previous: true,
            raise: true,
            quit: true,
        },
        shuffle: Some(false),
        loop_mode: Some(LoopMode::Off),
        seek_mode: SeekMode::Relative,
    }
}

fn paused_video(clock: Clock) -> Media {
    Media {
        bus: "org.mpris.MediaPlayer2.firefox.instance_1_42".to_owned(),
        identity: "Mozilla Firefox".to_owned(),
        desktop_entry: Some("firefox".to_owned()),
        title: Some("Lofi beats to study to".to_owned()),
        artists: vec!["Lofi Girl".to_owned()],
        album: None,
        status: Status::Paused,
        progress: Progress::Timed {
            position: Duration::from_secs(1312),
            sampled_at: clock.instant,
            length: Duration::from_secs(3600),
            rate: 1.0,
        },
        shuffle: None,
        loop_mode: None,
        ..now_playing(clock)
    }
}
