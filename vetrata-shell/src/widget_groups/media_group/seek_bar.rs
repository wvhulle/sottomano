use std::time::{Duration, Instant};

use iced::{Element, mouse::ScrollDelta};
use vetrata_config::{Config, severity::Severity};

use crate::{
    data::source::mpris::{Command, Media, SeekMode, micros},
    widgets::{inline_slider::inline_slider, slider_control::scroll_direction},
};

const SCROLL_SEEK_STEP: Duration = Duration::from_secs(5);
const DRAG_STEP_SECONDS: f32 = 1.0;
const ARRIVAL_TOLERANCE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy)]
pub enum SeekMessage {
    Drag(f32),
    Release,
    Scroll(ScrollDelta),
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Arrival {
    position: Duration,
    seek_mode: SeekMode,
}

impl Arrival {
    fn settled_by(&self, media: &Media, now: Instant) -> bool {
        self.seek_mode != media.seek_mode
            || media
                .progress
                .played(media.status, now)
                .is_none_or(|played| is_near(played, self.position))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum UserRequest {
    #[default]
    FollowingPlayer,
    DraggingTo(Duration),
    AwaitingArrival(Arrival),
}

impl UserRequest {
    fn requested_position(&self) -> Option<Duration> {
        match self {
            Self::FollowingPlayer => None,
            Self::DraggingTo(position) | Self::AwaitingArrival(Arrival { position, .. }) => {
                Some(*position)
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct SeekBar {
    request: UserRequest,
    scroll_accumulated: f32,
}

impl SeekBar {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, message: SeekMessage, media: &Media, now: Instant) -> Option<Command> {
        match message {
            SeekMessage::Drag(seconds) => {
                self.request = UserRequest::DraggingTo(Duration::from_secs_f32(seconds.max(0.0)));
                None
            }
            SeekMessage::Release => self.released(media, now),
            SeekMessage::Scroll(delta) => self.scrolled(delta, media, now),
            SeekMessage::Rejected => {
                self.request = UserRequest::FollowingPlayer;
                None
            }
        }
    }

    pub fn player_reported(&mut self, media: &Media, now: Instant) {
        if let UserRequest::AwaitingArrival(arrival) = &self.request
            && arrival.settled_by(media, now)
        {
            self.request = UserRequest::FollowingPlayer;
        }
    }

    fn released(&mut self, media: &Media, now: Instant) -> Option<Command> {
        let UserRequest::DraggingTo(position) = self.request else {
            return None;
        };
        self.request_arrival(media, position, now)
    }

    fn scrolled(&mut self, delta: ScrollDelta, media: &Media, now: Instant) -> Option<Command> {
        let steps = scroll_direction(&mut self.scroll_accumulated, delta)?;
        let length = media.progress.length()?;
        let shown = self.shown_position(media, now)?;
        self.request_arrival(media, scrolled_position(shown, steps, length), now)
    }

    fn request_arrival(
        &mut self,
        media: &Media,
        position: Duration,
        now: Instant,
    ) -> Option<Command> {
        self.request = UserRequest::FollowingPlayer;
        let command = seek_to(media, position, now)?;
        self.request = UserRequest::AwaitingArrival(Arrival {
            position,
            seek_mode: media.seek_mode.clone(),
        });
        Some(command)
    }

    fn shown_position(&self, media: &Media, now: Instant) -> Option<Duration> {
        self.request
            .requested_position()
            .or_else(|| media.progress.played(media.status, now))
    }

    fn lagging_position(&self, played: Duration) -> Option<Duration> {
        self.request
            .requested_position()
            .filter(|&requested| !is_near(played, requested))
            .map(|_| played)
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
        let length = media.progress.length()?;
        let played = media.progress.played(media.status, now)?;
        let shown = self.request.requested_position().unwrap_or(played);
        Some(inline_slider(
            config,
            0.0..=length.as_secs_f32(),
            shown.as_secs_f32(),
            self.lagging_position(played)
                .map(|lagging| lagging.as_secs_f32()),
            DRAG_STEP_SECONDS,
            Severity::Normal,
            SeekMessage::Drag,
            SeekMessage::Release,
            SeekMessage::Scroll,
        ))
    }
}

fn is_near(played: Duration, requested: Duration) -> bool {
    played.abs_diff(requested) <= ARRIVAL_TOLERANCE
}

fn scrolled_position(shown: Duration, steps: i64, length: Duration) -> Duration {
    let target = micros(shown).saturating_add(steps.saturating_mul(micros(SCROLL_SEEK_STEP)));
    Duration::from_micros(u64::try_from(target).unwrap_or(0)).min(length)
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
    use zbus::zvariant::{ObjectPath, OwnedObjectPath};

    use super::*;
    use crate::data::source::{
        bus::OwnerProcess,
        mpris::{Capabilities, Progress, Status},
    };

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
            artist: None,
            album: None,
            url: None,
            status: Status::Paused,
            progress: Progress::Timed {
                position,
                sampled_at: now,
                length: Duration::from_secs(300),
                rate: 1.0,
            },
            art: None,
            capabilities: Capabilities {
                seek: true,
                ..Capabilities::default()
            },
            shuffle: None,
            loop_mode: None,
            seek_mode,
        }
    }

    fn released_at(target: Duration, media: &Media, now: Instant) -> SeekBar {
        let mut bar = SeekBar::default();
        bar.update(SeekMessage::Drag(target.as_secs_f32()), media, now);
        let command = bar.update(SeekMessage::Release, media, now);
        assert!(matches!(command, Some(Command::SetPosition(_, position)) if position == target));
        bar
    }

    #[test]
    fn released_seek_keeps_target_until_the_player_arrives() {
        let now = Instant::now();
        let target = Duration::from_secs(120);
        let before = paused_at(Duration::from_secs(10), track("/t/1"), now);
        let mut bar = released_at(target, &before, now);
        assert_eq!(bar.shown_position(&before, now), Some(target));
        assert_eq!(
            bar.lagging_position(Duration::from_secs(10)),
            Some(Duration::from_secs(10))
        );

        bar.player_reported(&before, now);
        assert_eq!(bar.request.requested_position(), Some(target));

        let arrived = paused_at(target, track("/t/1"), now);
        bar.player_reported(&arrived, now);
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn track_change_abandons_the_seek() {
        let now = Instant::now();
        let before = paused_at(Duration::from_secs(10), track("/t/1"), now);
        let mut bar = released_at(Duration::from_secs(120), &before, now);
        bar.player_reported(&paused_at(Duration::ZERO, track("/t/2"), now), now);
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn rejected_seek_follows_the_player_again() {
        let now = Instant::now();
        let before = paused_at(Duration::from_secs(10), track("/t/1"), now);
        let mut bar = released_at(Duration::from_secs(120), &before, now);
        bar.update(SeekMessage::Rejected, &before, now);
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn scrolling_accumulates_from_the_requested_position() {
        let now = Instant::now();
        let media = paused_at(Duration::from_secs(10), track("/t/1"), now);
        let mut bar = SeekBar::default();
        let forward = ScrollDelta::Lines { x: 0.0, y: 1.0 };
        bar.update(SeekMessage::Scroll(forward), &media, now);
        bar.update(SeekMessage::Scroll(forward), &media, now);
        assert_eq!(
            bar.request.requested_position(),
            Some(Duration::from_secs(20))
        );
    }

    #[test]
    fn scrolled_position_stays_within_the_track() {
        let length = Duration::from_secs(60);
        assert_eq!(
            scrolled_position(Duration::from_secs(2), -1, length),
            Duration::ZERO
        );
        assert_eq!(
            scrolled_position(Duration::from_secs(58), 1, length),
            length
        );
    }
}
