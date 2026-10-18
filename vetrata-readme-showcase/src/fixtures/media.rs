use std::time::{Duration, Instant};

use vetrata_sources::{
    SourceDelivery,
    bus::OwnerProcess,
    mpris::{Capabilities, LoopMode, Media, Progress, SeekMode, Status},
};
use vetrata_widgets::{
    desktop_index::DesktopIndex,
    shell_message::Message,
    widget_groups::media_group::{MediaGroup, MediaMessage},
};

use crate::showcased_group::{ShowcasedGroup, ShowcasedMedia};

pub fn live(_tick: u32) -> Vec<Message> {
    vec![Message::Media(MediaMessage::Tick(Instant::now()))]
}

pub fn group() -> ShowcasedGroup {
    let mut group = MediaGroup::default();
    let _ = group.update(MediaMessage::Tick(Instant::now()));
    let _ = group.update(MediaMessage::Reading(SourceDelivery::Reading(Ok(vec![
        now_playing(),
    ]))));
    ShowcasedGroup::Media(Box::new(ShowcasedMedia {
        group,
        desktop_entries: DesktopIndex::default(),
    }))
}

fn now_playing() -> Media {
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
            sampled_at: Instant::now(),
            length: Duration::from_secs(241),
            rate: 1.0,
        },
        art: None,
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
