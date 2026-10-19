use std::time::Duration;

use sottomano_sources::{
    SourceDelivery,
    media_playback_audio::mpris_players::{
        Capabilities, LoopMode, Media, Progress, SeekMode, Status,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    widget_groups::media_group::{MediaGroup, MediaMessage},
};

use crate::showcased_group::{ShowcasedGroup, ShowcasedMedia};

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = MediaGroup::default();
    let _ = group.update(
        MediaMessage::Reading(SourceDelivery::Reading(Ok(vec![now_playing(clock)]))),
        clock,
    );
    ShowcasedGroup::Media(Box::new(ShowcasedMedia {
        group,
        desktop_entries: DesktopIndex::default(),
    }))
}

fn now_playing(clock: Clock) -> Media {
    Media {
        bus: "org.mpris.MediaPlayer2.spotify".to_owned(),
        owner: OwnerProcess::Unidentified,
        identity: "Spotify".to_owned(),
        desktop_entry: Some("spotify".to_owned()),
        title: Some("Drifting".to_owned()),
        artist: Some("Shir-Ran Yinon".to_owned()),
        album: Some("Fiddler's Path".to_owned()),
        url: None,
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
