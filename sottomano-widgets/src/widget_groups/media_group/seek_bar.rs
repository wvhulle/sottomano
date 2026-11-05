use std::time::{Duration, Instant};

pub use corredo::seek_bar::SeekMessage;
use corredo::seek_bar::{PlaybackTimeline, SeekBar as PlaybackSeekBar};
use iced::Element;
use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, Media, SeekMode, micros,
};
use sottomano_toml_config::Config;

#[derive(Debug, Default)]
pub struct SeekBar {
    playback: PlaybackSeekBar<SeekMode>,
}

fn timeline_of(media: &Media, now: Instant) -> PlaybackTimeline<SeekMode> {
    let played = media.progress.played(media.status, now);
    PlaybackTimeline {
        played,
        length: media.progress.length(),
        session: media.seek_mode.clone(),
        accepts_seek: played.is_some() || matches!(media.seek_mode, SeekMode::Absolute(_)),
    }
}

impl SeekBar {
    pub fn update(&mut self, message: SeekMessage, media: &Media, now: Instant) -> Option<Command> {
        let target = self.playback.update(message, &timeline_of(media, now))?;
        seek_to(media, target, now)
    }

    pub fn player_reported(&mut self, media: &Media, now: Instant) {
        self.playback.player_reported(&timeline_of(media, now));
    }

    pub fn seekable(media: &Media, now: Instant) -> bool {
        media.capabilities.seek
            && media.progress.length().is_some()
            && media.progress.played(media.status, now).is_some()
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        media: &Media,
        now: Instant,
    ) -> Option<Element<'a, SeekMessage>> {
        if !media.capabilities.seek {
            return None;
        }
        self.playback
            .view(config.theme.font.icon_size_px, &timeline_of(media, now))
    }
}

fn seek_to(media: &Media, target: Duration, now: Instant) -> Option<Command> {
    match &media.seek_mode {
        SeekMode::Absolute(track) => Some(Command::SetPosition(track.clone(), target)),
        SeekMode::Relative => {
            let played = media.progress.played(media.status, now)?;
            Some(Command::Seek(micros(target) - micros(played)))
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::{
        media_playback_audio::mpris_players::{Capabilities, Progress, Status},
        system_protocol_clients::dbus_desktop_buses::OwnerProcess,
    };
    use zbus::zvariant::{ObjectPath, OwnedObjectPath};

    use super::*;
    use crate::clock::Clock;

    fn track(path: &str) -> SeekMode {
        SeekMode::Absolute(OwnedObjectPath::from(ObjectPath::try_from(path).unwrap()))
    }

    fn paused_at(position: Duration, seek_mode: SeekMode, now: Instant) -> Media {
        Media {
            bus: "org.mpris.MediaPlayer2.test".to_owned(),
            owner: OwnerProcess::Unidentified,
            identity: "test".to_owned(),
            desktop_entry: None,
            title: Some("track".to_owned()),
            artists: Vec::new(),
            album: None,
            url: None,
            status: Status::Paused,
            progress: Progress::Timed {
                position,
                sampled_at: now,
                length: Duration::from_secs(300),
                rate: 1.0,
            },
            art_url: None,
            capabilities: Capabilities {
                seek: true,
                ..Capabilities::default()
            },
            shuffle: None,
            loop_mode: None,
            seek_mode,
        }
    }

    #[test]
    fn a_released_drag_sets_the_position_of_the_track() {
        let now = Clock::real().instant;
        let target = Duration::from_secs(120);
        let media = paused_at(Duration::from_secs(10), track("/t/1"), now);
        let mut bar = SeekBar::default();
        bar.update(SeekMessage::Drag(target.as_secs_f32()), &media, now);
        let command = bar.update(SeekMessage::Release, &media, now);
        assert!(matches!(command, Some(Command::SetPosition(_, position)) if position == target));
    }
}
