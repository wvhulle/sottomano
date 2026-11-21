use std::convert::identity;

use iced::{
    Animation,
    time::{Duration, Instant},
};

use crate::{
    pointer_hover::PointerHover,
    transitions::{MotionState, combined_motion, eased_animation},
};

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
    pub pointer_hover: PointerHover,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HoverHold {
    Released,
    HeldAt(f32),
}

pub(super) struct ScrollMotion {
    pacing: ScrollPacing,
    travel: Animation<f32>,
    pub(super) target_offset: f32,
    pub(super) drawn_offset: f32,
    hover_hold: HoverHold,
}

impl ScrollMotion {
    pub(super) fn resting_at(offset: f32, pacing: ScrollPacing) -> Self {
        Self {
            pacing,
            travel: travel_resting_at(offset, pacing),
            target_offset: offset,
            drawn_offset: offset,
            hover_hold: HoverHold::Released,
        }
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
        combined_motion([&self.travel], now)
    }

    pub(super) fn hover_drift(&self) -> f32 {
        match self.hover_hold {
            HoverHold::Released => 0.0,
            HoverHold::HeldAt(held_offset) => held_offset - self.drawn_offset,
        }
    }

    pub(super) fn advance(&mut self, frame: ScrollFrame) -> OffsetChange {
        let ScrollFrame {
            now,
            max_offset,
            pointer_hover,
        } = frame;
        let previously_drawn = self.drawn_offset;
        self.target_offset = self.target_offset.clamp(0.0, max_offset);
        self.settle_if_instant(max_offset);
        if (self.travel.value() - self.target_offset).abs() > f32::EPSILON {
            if !self.travel.is_animating(now) {
                self.hover_hold = match pointer_hover {
                    PointerHover::Over => HoverHold::HeldAt(self.drawn_offset),
                    PointerHover::Outside => HoverHold::Released,
                };
            }
            self.travel.go_mut(self.target_offset, now);
        }
        self.drawn_offset = self
            .travel
            .interpolate_with(identity, now)
            .clamp(0.0, max_offset);
        self.hover_hold = match (self.hover_hold, pointer_hover) {
            (HoverHold::HeldAt(held_offset), PointerHover::Over)
                if self.travel.is_animating(now) =>
            {
                HoverHold::HeldAt(held_offset.clamp(0.0, max_offset))
            }
            (HoverHold::HeldAt(_) | HoverHold::Released, _) => HoverHold::Released,
        };
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
    use crate::pointer_hover::PointerHover;

    const TRAVEL_DURATION: Duration = Duration::from_millis(160);
    const EASED: ScrollPacing = ScrollPacing::Eased(TRAVEL_DURATION);
    const MAX_OFFSET: f32 = 1000.0;

    fn frame_at(now: Instant) -> ScrollFrame {
        ScrollFrame {
            now,
            max_offset: MAX_OFFSET,
            pointer_hover: PointerHover::Over,
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
        assert!(close_to(motion.drawn_offset, 0.0));
        let _midway = motion.advance(frame_at(start + TRAVEL_DURATION / 2));
        assert!(motion.drawn_offset > 0.0 && motion.drawn_offset < 100.0);
        let _arrived = motion.advance(frame_at(start + TRAVEL_DURATION));
        assert!(close_to(motion.drawn_offset, 100.0));
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
            assert!(close_to(motion.drawn_offset, 70.0));
            assert_eq!(motion.travel_state(now), MotionState::Settled);
        }
    }

    #[test]
    fn a_jump_applies_at_once() {
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.jump_to(70.0);
        assert!(close_to(motion.drawn_offset, 70.0));
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
        assert!(close_to(motion.hover_drift(), -motion.drawn_offset));
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
        assert!(close_to(motion.hover_drift(), -motion.drawn_offset));
    }

    #[test]
    fn a_pointer_outside_when_motion_starts_holds_nothing() {
        let start = Instant::now();
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(100.0);
        let _started = motion.advance(ScrollFrame {
            pointer_hover: PointerHover::Outside,
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
            pointer_hover: PointerHover::Outside,
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
        assert!(close_to(motion.target_offset, 100.0));
        assert!(motion.drawn_offset <= 100.0);
        assert!(close_to(motion.hover_drift(), 0.0));
    }

    #[test]
    fn changed_pacing_takes_effect_from_the_target() {
        let mut motion = ScrollMotion::resting_at(0.0, EASED);
        motion.set_target(60.0);
        motion.apply_pacing(ScrollPacing::Instant);
        assert!(close_to(motion.drawn_offset, 60.0));
    }
}
