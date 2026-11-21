use std::time::Duration;

use iced::{Element, mouse::ScrollDelta};

use crate::{
    selection_scroll::wheel_step_direction,
    value_controls::{
        inline_slider,
        synced_value::{SourceLag, SyncedValue, ValueWrite},
    },
};

const SCROLL_SEEK_STEP: Duration = Duration::from_secs(5);
const DRAG_STEP_SECONDS: f32 = 1.0;
const ARRIVAL_TOLERANCE: Duration = Duration::from_secs(1);
const ARRIVAL_STEP_SECONDS: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
pub enum SeekMessage {
    Drag(f32),
    Release,
    Scroll(ScrollDelta),
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackTimeline<PlaybackSession> {
    pub progress: PlaybackProgress,
    pub session: PlaybackSession,
    pub seek_acceptance: SeekAcceptance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackProgress {
    Untimed,
    Timed { played: Duration, length: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekAcceptance {
    Accepted,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SeekSession<PlaybackSession> {
    NoSeek,
    SeekingIn(PlaybackSession),
}

#[derive(Debug)]
pub struct SeekBar<PlaybackSession> {
    synced_position: SyncedValue,
    seek_session: SeekSession<PlaybackSession>,
    scroll_accumulated: f32,
}

impl<PlaybackSession> Default for SeekBar<PlaybackSession> {
    fn default() -> Self {
        Self {
            synced_position: SyncedValue::writing_on_release(),
            seek_session: SeekSession::NoSeek,
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
                let _no_write_while_dragging = self.synced_position.drag_moved_to(seconds.max(0.0));
                None
            }
            SeekMessage::Release => {
                let release_write = self.synced_position.drag_ended();
                self.seek_with(release_write, timeline)
            }
            SeekMessage::Scroll(delta) => self.scrolled(delta, timeline),
            SeekMessage::Rejected => {
                self.follow_player();
                None
            }
        }
    }

    pub fn player_reported(&mut self, timeline: &PlaybackTimeline<PlaybackSession>) {
        let left_session = match &self.seek_session {
            SeekSession::SeekingIn(session) => *session != timeline.session,
            SeekSession::NoSeek => false,
        };
        match timeline.progress {
            PlaybackProgress::Timed { played, .. } if !left_session => self
                .synced_position
                .source_reported(played.as_secs_f32(), ARRIVAL_STEP_SECONDS),
            PlaybackProgress::Timed { .. } | PlaybackProgress::Untimed => self.follow_player(),
        }
    }

    fn scrolled(
        &mut self,
        delta: ScrollDelta,
        timeline: &PlaybackTimeline<PlaybackSession>,
    ) -> Option<Duration> {
        let steps = wheel_step_direction(&mut self.scroll_accumulated, delta)?;
        let PlaybackProgress::Timed { played, length } = timeline.progress else {
            return None;
        };
        let target = scrolled_position(self.shown_position(played), steps, length);
        let scroll_write = self.synced_position.scrolled_to(target.as_secs_f32());
        self.seek_with(scroll_write, timeline)
    }

    fn seek_with(
        &mut self,
        position_write: Option<ValueWrite>,
        timeline: &PlaybackTimeline<PlaybackSession>,
    ) -> Option<Duration> {
        let Some(ValueWrite::Absolute(seconds)) = position_write else {
            return None;
        };
        if timeline.seek_acceptance == SeekAcceptance::Refused {
            self.follow_player();
            return None;
        }
        let _seek_sent_without_confirmation = self
            .synced_position
            .absolute_write_finished(seconds, &Ok::<(), ()>(()));
        self.seek_session = SeekSession::SeekingIn(timeline.session.clone());
        Some(Duration::from_secs_f32(seconds))
    }

    fn follow_player(&mut self) {
        self.synced_position.follow_source();
        self.seek_session = SeekSession::NoSeek;
    }

    fn requested_position(&self) -> Option<Duration> {
        self.synced_position
            .user_request
            .requested_value()
            .map(Duration::from_secs_f32)
    }

    fn shown_position(&self, played: Duration) -> Duration {
        self.requested_position().unwrap_or(played)
    }

    fn lagging_position(&self, played: Duration) -> SourceLag {
        match self.requested_position() {
            Some(requested) if played.abs_diff(requested) > ARRIVAL_TOLERANCE => {
                SourceLag::LaggingAt(played.as_secs_f32())
            }
            Some(_) | None => SourceLag::CaughtUp,
        }
    }

    pub fn view<'view>(
        &self,
        icon_size_px: f32,
        timeline: &PlaybackTimeline<PlaybackSession>,
    ) -> Option<Element<'view, SeekMessage>> {
        let PlaybackProgress::Timed { played, length } = timeline.progress else {
            return None;
        };
        let shown = self.shown_position(played);
        Some(
            inline_slider(
                0.0..=length.as_secs_f32(),
                shown.as_secs_f32(),
                SeekMessage::Drag,
            )
            .icon_size(icon_size_px)
            .lagging_behind(self.lagging_position(played))
            .step(DRAG_STEP_SECONDS)
            .on_release(SeekMessage::Release)
            .on_scroll(SeekMessage::Scroll)
            .into(),
        )
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "scrolling back past the start seeks to the start"
)]
fn scrolled_position(shown: Duration, steps: i64, length: Duration) -> Duration {
    let seek_distance = SCROLL_SEEK_STEP
        * u32::try_from(steps.unsigned_abs())
            .expect("a scroll gesture moves fewer than u32::MAX steps");
    if steps >= 0 {
        (shown + seek_distance).min(length)
    } else {
        shown.saturating_sub(seek_distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTH: Duration = Duration::from_secs(300);
    const FIRST_TRACK: u32 = 1;
    const SECOND_TRACK: u32 = 2;

    fn timeline_at(played: Duration, session: u32) -> PlaybackTimeline<u32> {
        PlaybackTimeline {
            progress: PlaybackProgress::Timed {
                played,
                length: LENGTH,
            },
            session,
            seek_acceptance: SeekAcceptance::Accepted,
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
        assert_eq!(bar.shown_position(Duration::from_secs(10)), target);
        assert_eq!(
            bar.lagging_position(Duration::from_secs(10)),
            SourceLag::LaggingAt(10.0)
        );

        bar.player_reported(&before);
        assert_eq!(bar.requested_position(), Some(target));

        bar.player_reported(&timeline_at(target, FIRST_TRACK));
        assert_eq!(bar.requested_position(), None);
    }

    #[test]
    fn session_change_abandons_the_seek() {
        let before = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = released_at(Duration::from_secs(120), &before);
        bar.player_reported(&timeline_at(Duration::ZERO, SECOND_TRACK));
        assert_eq!(bar.requested_position(), None);
    }

    #[test]
    fn rejected_seek_follows_the_player_again() {
        let before = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = released_at(Duration::from_secs(120), &before);
        bar.update(SeekMessage::Rejected, &before);
        assert_eq!(bar.requested_position(), None);
    }

    #[test]
    fn a_timeline_refusing_seeks_keeps_following_the_player() {
        let refusing = PlaybackTimeline {
            seek_acceptance: SeekAcceptance::Refused,
            ..timeline_at(Duration::from_secs(10), FIRST_TRACK)
        };
        let mut bar = SeekBar::default();
        bar.update(SeekMessage::Drag(120.0), &refusing);
        assert_eq!(bar.update(SeekMessage::Release, &refusing), None);
        assert_eq!(bar.requested_position(), None);
    }

    #[test]
    fn scrolling_accumulates_from_the_requested_position() {
        let timeline = timeline_at(Duration::from_secs(10), FIRST_TRACK);
        let mut bar = SeekBar::default();
        let forward = ScrollDelta::Lines { x: 0.0, y: 1.0 };
        bar.update(SeekMessage::Scroll(forward), &timeline);
        bar.update(SeekMessage::Scroll(forward), &timeline);
        assert_eq!(bar.requested_position(), Some(Duration::from_secs(20)));
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
