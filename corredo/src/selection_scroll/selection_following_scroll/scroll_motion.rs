use std::convert::identity;

use iced::{
    Animation,
    time::{Duration, Instant},
};

use crate::transitions::{MotionState, eased_animation, motion_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPacing {
    Instant,
    Eased(Duration),
}

impl ScrollPacing {
    fn travel_duration(self) -> Duration {
        match self {
            Self::Instant => Duration::ZERO,
            Self::Eased(duration) => duration,
        }
    }
}

fn travel_resting_at(offset: f32, pacing: ScrollPacing) -> Animation<f32> {
    eased_animation(offset, pacing.travel_duration())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OffsetChange {
    Moved,
    Unchanged,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScrollFrame {
    pub now: Instant,
    pub max_offset: f32,
    pub pointer_inside: bool,
}

pub(super) struct ScrollMotion {
    pacing: ScrollPacing,
    travel: Animation<f32>,
    target_offset: f32,
    drawn_offset: f32,
    hover_held_offset: Option<f32>,
}

impl ScrollMotion {
    pub(super) fn resting_at(offset: f32, pacing: ScrollPacing) -> Self {
        Self {
            pacing,
            travel: travel_resting_at(offset, pacing),
            target_offset: offset,
            drawn_offset: offset,
            hover_held_offset: None,
        }
    }

    pub(super) fn target_offset(&self) -> f32 {
        self.target_offset
    }

    pub(super) fn drawn_offset(&self) -> f32 {
        self.drawn_offset
    }

    pub(super) fn set_target(&mut self, offset: f32) {
        self.target_offset = offset;
    }

    pub(super) fn jump_to(&mut self, offset: f32) {
        *self = Self::resting_at(offset, self.pacing);
    }

    pub(super) fn apply_pacing(&mut self, pacing: ScrollPacing) {
        if self.pacing != pacing {
            *self = Self::resting_at(self.target_offset, pacing);
        }
    }

    pub(super) fn settle_if_instant(&mut self, max_offset: f32) {
        if self.pacing.travel_duration().is_zero() {
            self.jump_to(self.target_offset.clamp(0.0, max_offset));
        }
    }

    pub(super) fn travel_state(&self, now: Instant) -> MotionState {
        motion_of([&self.travel], now)
    }

    pub(super) fn hover_drift(&self) -> f32 {
        self.hover_held_offset
            .map_or(0.0, |held_offset| held_offset - self.drawn_offset)
    }

    pub(super) fn advance(&mut self, frame: ScrollFrame) -> OffsetChange {
        let ScrollFrame {
            now,
            max_offset,
            pointer_inside,
        } = frame;
        let previously_drawn = self.drawn_offset;
        self.target_offset = self.target_offset.clamp(0.0, max_offset);
        self.settle_if_instant(max_offset);
        if (self.travel.value() - self.target_offset).abs() > f32::EPSILON {
            if !self.travel.is_animating(now) {
                self.hover_held_offset = pointer_inside.then_some(self.drawn_offset);
            }
            self.travel.go_mut(self.target_offset, now);
        }
        self.drawn_offset = self
            .travel
            .interpolate_with(identity, now)
            .clamp(0.0, max_offset);
        self.hover_held_offset = self
            .hover_held_offset
            .filter(|_| pointer_inside && self.travel.is_animating(now))
            .map(|held_offset| held_offset.clamp(0.0, max_offset));
        if (self.drawn_offset - previously_drawn).abs() > f32::EPSILON {
            OffsetChange::Moved
        } else {
            OffsetChange::Unchanged
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use iced::time::{Duration, Instant};

    use super::{MotionState, OffsetChange, ScrollFrame, ScrollMotion, ScrollPacing};

    const TRAVEL_DURATION: Duration = Duration::from_millis(160);
    const EASED: ScrollPacing = ScrollPacing::Eased(TRAVEL_DURATION);
    const MAX_OFFSET: f32 = 1000.0;

    fn frame_at(now: Instant) -> ScrollFrame {
        ScrollFrame {
            now,
            max_offset: MAX_OFFSET,
            pointer_inside: true,
        }
    }

    fn close_to(first: f32, second: f32) -> bool {
        (first - second).abs() < 1e-3
    }

    #[test]
    fn a_new_offset_is_reached_gradually() {
        let start = Instant::now();
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(100.0);
        let _started = motion.advance(frame_at(start));
        assert!(close_to(motion.drawn_offset(), 0.0));
        let _midway = motion.advance(frame_at(start + TRAVEL_DURATION / 2));
        assert!(motion.drawn_offset() > 0.0 && motion.drawn_offset() < 100.0);
        let _arrived = motion.advance(frame_at(start + TRAVEL_DURATION));
        assert!(close_to(motion.drawn_offset(), 100.0));
    }

    #[test]
    fn a_resting_list_reports_no_movement() {
        let mut motion = ScrollMotion::resting_at(40.0, EASED);
        assert_eq!(
            motion.advance(frame_at(Instant::now())),
            OffsetChange::Unchanged
        );
    }

    #[test]
    fn instant_and_zero_length_pacing_apply_at_once() {
        for pacing in [ScrollPacing::Instant, ScrollPacing::Eased(Duration::ZERO)] {
            let now = Instant::now();
            let mut motion = ScrollMotion::resting_at(0.0, pacing);
            motion.set_target(70.0);
            assert_eq!(motion.advance(frame_at(now)), OffsetChange::Moved);
            assert!(close_to(motion.drawn_offset(), 70.0));
            assert_eq!(motion.travel_state(now), MotionState::Settled);
        }
    }

    #[test]
    fn a_jump_applies_at_once() {
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.jump_to(70.0);
        assert!(close_to(motion.drawn_offset(), 70.0));
        assert_eq!(
            motion.advance(frame_at(Instant::now())),
            OffsetChange::Unchanged
        );
    }

    #[test]
    fn hover_stays_on_the_same_content_until_the_scroll_settles() {
        let start = Instant::now();
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(100.0);
        let _started = motion.advance(frame_at(start));
        let _midway = motion.advance(frame_at(start + TRAVEL_DURATION / 2));
        assert!(close_to(motion.hover_drift(), -motion.drawn_offset()));
        let _arrived = motion.advance(frame_at(start + TRAVEL_DURATION));
        assert!(close_to(motion.hover_drift(), 0.0));
    }

    #[test]
    fn retargeting_mid_scroll_keeps_holding_the_first_content() {
        let start = Instant::now();
        let midway = start + TRAVEL_DURATION / 2;
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(100.0);
        let _started = motion.advance(frame_at(start));
        let _midway = motion.advance(frame_at(midway));
        motion.set_target(200.0);
        let _retargeted = motion.advance(frame_at(midway));
        assert!(close_to(motion.hover_drift(), -motion.drawn_offset()));
    }

    #[test]
    fn a_pointer_outside_when_motion_starts_holds_nothing() {
        let start = Instant::now();
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(100.0);
        let _started = motion.advance(ScrollFrame {
            pointer_inside: false,
            ..frame_at(start)
        });
        let _entered_midway = motion.advance(frame_at(start + TRAVEL_DURATION / 2));
        assert!(close_to(motion.hover_drift(), 0.0));
    }

    #[test]
    fn a_pointer_leaving_mid_scroll_releases_the_hold() {
        let start = Instant::now();
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(100.0);
        let _started = motion.advance(frame_at(start));
        let _left = motion.advance(ScrollFrame {
            pointer_inside: false,
            ..frame_at(start + TRAVEL_DURATION / 4)
        });
        let _returned = motion.advance(frame_at(start + TRAVEL_DURATION / 2));
        assert!(close_to(motion.hover_drift(), 0.0));
    }

    #[test]
    fn a_shrinking_list_clamps_target_drawn_and_held_offsets() {
        let start = Instant::now();
        let mut motion = ScrollMotion::resting_at(500.0, EASED);
        motion.set_target(800.0);
        let _started = motion.advance(frame_at(start));
        let shrunk = ScrollFrame {
            max_offset: 100.0,
            ..frame_at(start + TRAVEL_DURATION / 2)
        };
        let _shrunk = motion.advance(shrunk);
        assert!(close_to(motion.target_offset(), 100.0));
        assert!(motion.drawn_offset() <= 100.0);
        assert!(close_to(motion.hover_drift(), 0.0));
    }

    #[test]
    fn changed_pacing_takes_effect_from_the_target() {
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(60.0);
        motion.apply_pacing(ScrollPacing::Instant);
        assert!(close_to(motion.drawn_offset(), 60.0));
    }
}
