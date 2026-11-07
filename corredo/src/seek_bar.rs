use std::time::Duration;

use iced::{Element, mouse::ScrollDelta};

use crate::{inline_slider::inline_slider, scroll_steps::scroll_direction};

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
pub struct PlaybackTimeline<PlaybackSession> {
    pub played: Option<Duration>,
    pub length: Option<Duration>,
    pub session: PlaybackSession,
    pub accepts_seek: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeekArrival {
    Arrived,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Arrival<PlaybackSession> {
    position: Duration,
    session: PlaybackSession,
}

impl<PlaybackSession: PartialEq> Arrival<PlaybackSession> {
    fn arrival_state(&self, timeline: &PlaybackTimeline<PlaybackSession>) -> SeekArrival {
        if self.session != timeline.session
            || timeline
                .played
                .is_none_or(|played| played.abs_diff(self.position) <= ARRIVAL_TOLERANCE)
        {
            SeekArrival::Arrived
        } else {
            SeekArrival::Pending
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum UserRequest<PlaybackSession> {
    FollowingPlayer,
    DraggingTo(Duration),
    AwaitingArrival(Arrival<PlaybackSession>),
}

impl<PlaybackSession> UserRequest<PlaybackSession> {
    fn requested_position(&self) -> Option<Duration> {
        match self {
            Self::FollowingPlayer => None,
            Self::DraggingTo(position) | Self::AwaitingArrival(Arrival { position, .. }) => {
                Some(*position)
            }
        }
    }
}

#[derive(Debug)]
pub struct SeekBar<PlaybackSession> {
    request: UserRequest<PlaybackSession>,
    scroll_accumulated: f32,
}

impl<PlaybackSession> Default for SeekBar<PlaybackSession> {
    fn default() -> Self {
        Self {
            request: UserRequest::FollowingPlayer,
            scroll_accumulated: 0.0,
        }
    }
}

impl<PlaybackSession: Clone + PartialEq> SeekBar<PlaybackSession> {
    pub fn update(
        &mut self,
        message: SeekMessage,
        timeline: &PlaybackTimeline<PlaybackSession>,
    ) -> Option<Duration> {
        match message {
            SeekMessage::Drag(seconds) => {
                self.request = UserRequest::DraggingTo(Duration::from_secs_f32(seconds.max(0.0)));
                None
            }
            SeekMessage::Release => self.released(timeline),
            SeekMessage::Scroll(delta) => self.scrolled(delta, timeline),
            SeekMessage::Rejected => {
                self.request = UserRequest::FollowingPlayer;
                None
            }
        }
    }

    pub fn player_reported(&mut self, timeline: &PlaybackTimeline<PlaybackSession>) {
        if let UserRequest::AwaitingArrival(arrival) = &self.request
            && arrival.arrival_state(timeline) == SeekArrival::Arrived
        {
            self.request = UserRequest::FollowingPlayer;
        }
    }

    fn released(&mut self, timeline: &PlaybackTimeline<PlaybackSession>) -> Option<Duration> {
        let UserRequest::DraggingTo(position) = self.request else {
            return None;
        };
        self.request_arrival(timeline, position)
    }

    fn scrolled(
        &mut self,
        delta: ScrollDelta,
        timeline: &PlaybackTimeline<PlaybackSession>,
    ) -> Option<Duration> {
        let steps = scroll_direction(&mut self.scroll_accumulated, delta)?;
        let length = timeline.length?;
        let shown = self.shown_position(timeline)?;
        self.request_arrival(timeline, scrolled_position(shown, steps, length))
    }

    fn request_arrival(
        &mut self,
        timeline: &PlaybackTimeline<PlaybackSession>,
        position: Duration,
    ) -> Option<Duration> {
        self.request = UserRequest::FollowingPlayer;
        if !timeline.accepts_seek {
            return None;
        }
        self.request = UserRequest::AwaitingArrival(Arrival {
            position,
            session: timeline.session.clone(),
        });
        Some(position)
    }

    fn shown_position(&self, timeline: &PlaybackTimeline<PlaybackSession>) -> Option<Duration> {
        self.request.requested_position().or(timeline.played)
    }

    fn lagging_position(&self, played: Duration) -> Option<Duration> {
        self.request
            .requested_position()
            .filter(|&requested| played.abs_diff(requested) > ARRIVAL_TOLERANCE)
            .map(|_| played)
    }

    pub fn view<'a>(
        &self,
        icon_size_px: f32,
        timeline: &PlaybackTimeline<PlaybackSession>,
    ) -> Option<Element<'a, SeekMessage>> {
        let length = timeline.length?;
        let played = timeline.played?;
        let shown = self.request.requested_position().unwrap_or(played);
        Some(
            inline_slider(
                0.0..=length.as_secs_f32(),
                shown.as_secs_f32(),
                SeekMessage::Drag,
            )
            .icon_size(icon_size_px)
            .lagging_behind(
                self.lagging_position(played)
                    .map(|lagging| lagging.as_secs_f32()),
            )
            .step(DRAG_STEP_SECONDS)
            .on_release(SeekMessage::Release)
            .on_scroll(SeekMessage::Scroll)
            .into(),
        )
    }
}

fn signed_micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

fn scrolled_position(shown: Duration, steps: i64, length: Duration) -> Duration {
    let target =
        signed_micros(shown).saturating_add(steps.saturating_mul(signed_micros(SCROLL_SEEK_STEP)));
    Duration::from_micros(u64::try_from(target).unwrap_or(0)).min(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTH: Duration = Duration::from_secs(300);
    const FIRST_TRACK: u32 = 1;
    const SECOND_TRACK: u32 = 2;

    fn timeline_at(played: Duration, session: u32) -> PlaybackTimeline<u32> {
        PlaybackTimeline {
            played: Some(played),
            length: Some(LENGTH),
            session,
            accepts_seek: true,
        }
    }

    fn released_at(target: Duration, timeline: &PlaybackTimeline<u32>) -> SeekBar<u32> {
        let mut bar = SeekBar::default();
        bar.update(SeekMessage::Drag(target.as_secs_f32()), timeline);
        assert_eq!(bar.update(SeekMessage::Release, timeline), Some(target));
        bar
    }

    #[test]
    fn released_seek_keeps_target_until_the_player_arrives() {
        let target = Duration::from_secs(120);
        let before = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = released_at(target, &before);
        assert_eq!(bar.shown_position(&before), Some(target));
        assert_eq!(
            bar.lagging_position(Duration::from_secs(10)),
            Some(Duration::from_secs(10))
        );

        bar.player_reported(&before);
        assert_eq!(bar.request.requested_position(), Some(target));

        bar.player_reported(&timeline_at(target, FIRST_TRACK));
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn session_change_abandons_the_seek() {
        let before = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = released_at(Duration::from_secs(120), &before);
        bar.player_reported(&timeline_at(Duration::ZERO, SECOND_TRACK));
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn rejected_seek_follows_the_player_again() {
        let before = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = released_at(Duration::from_secs(120), &before);
        bar.update(SeekMessage::Rejected, &before);
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn a_timeline_refusing_seeks_keeps_following_the_player() {
        let refusing = PlaybackTimeline {
            accepts_seek: false,
            ..timeline_at(Duration::from_secs(10), FIRST_TRACK)
        };
        let mut bar = SeekBar::default();
        bar.update(SeekMessage::Drag(120.0), &refusing);
        assert_eq!(bar.update(SeekMessage::Release, &refusing), None);
        assert_eq!(bar.request, UserRequest::FollowingPlayer);
    }

    #[test]
    fn scrolling_accumulates_from_the_requested_position() {
        let timeline = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = SeekBar::default();
        let forward = ScrollDelta::Lines { x: 0.0, y: 1.0 };
        bar.update(SeekMessage::Scroll(forward), &timeline);
        bar.update(SeekMessage::Scroll(forward), &timeline);
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
