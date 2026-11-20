use iced::{
    Animation, Rectangle, Theme,
    mouse::Cursor,
    time::{Duration, Instant},
};

use super::{
    slot_underlay::{PaintedRow, SlotPainter},
    travelling_highlight::{HighlightMark, fill_slot_mark},
};
use crate::{
    orientation::Axis,
    theming::background_strong_color,
    transitions::{MotionState, eased_animation, motion_of},
};

pub(super) struct HoverGlow {
    pub(super) pager_axis: Axis,
    pub(super) fade_duration: Duration,
    pub(super) slot_marks: Vec<HighlightMark>,
}

pub(super) struct SlotGlow {
    fade: Animation<bool>,
    level: f32,
}

impl HoverGlow {
    fn pointed_slot(&self, child_bounds: &[Rectangle], cursor: Cursor) -> Option<usize> {
        let pointer = cursor.position()?;
        child_bounds
            .iter()
            .take(self.slot_marks.len())
            .position(|slot| slot.contains(pointer))
    }
}

impl SlotPainter for HoverGlow {
    type PainterState = Vec<SlotGlow>;

    fn advance(
        &self,
        slot_glows: &mut Vec<SlotGlow>,
        child_bounds: &[Rectangle],
        cursor: Cursor,
        now: Instant,
    ) -> MotionState {
        let pointed_slot = self.pointed_slot(child_bounds, cursor);
        slot_glows.resize_with(self.slot_marks.len(), || SlotGlow {
            fade: eased_animation(false, self.fade_duration),
            level: 0.0,
        });
        for (slot_position, slot_glow) in slot_glows.iter_mut().enumerate() {
            slot_glow
                .fade
                .go_mut(pointed_slot == Some(slot_position), now);
            slot_glow.level = slot_glow.fade.interpolate(0.0, 1.0, now);
        }
        motion_of(slot_glows.iter().map(|slot_glow| &slot_glow.fade), now)
    }

    fn pointer_motion(
        &self,
        slot_glows: &Vec<SlotGlow>,
        child_bounds: &[Rectangle],
        cursor: Cursor,
    ) -> MotionState {
        let pointed_slot = self.pointed_slot(child_bounds, cursor);
        let glows_follow_pointer = (0..self.slot_marks.len()).all(|slot_position| {
            let pointed = pointed_slot == Some(slot_position);
            slot_glows
                .get(slot_position)
                .map_or(!pointed, |slot_glow| slot_glow.fade.value() == pointed)
        });
        if glows_follow_pointer {
            MotionState::Settled
        } else {
            MotionState::Moving
        }
    }

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, Vec<SlotGlow>>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        let glow_color = background_strong_color(theme);
        for ((slot_glow, slot_mark), slot) in painted_row
            .painter_state
            .iter()
            .zip(&self.slot_marks)
            .zip(painted_row.child_bounds)
            .filter(|((slot_glow, _), _)| slot_glow.level > 0.0)
        {
            fill_slot_mark(
                renderer,
                slot_mark,
                self.pager_axis,
                painted_row.row_bounds,
                *slot,
                glow_color.scale_alpha(slot_glow.level),
            );
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "an animation test needs a real starting instant"
)]
mod tests {
    use iced::{
        Point, Rectangle, Size,
        mouse::Cursor,
        time::{Duration, Instant},
    };

    use super::HoverGlow;
    use crate::{
        drag_reorder::reorderable_list::{
            slot_underlay::SlotPainter, travelling_highlight::HighlightMark,
        },
        orientation::{Axis, Edge},
        transitions::MotionState,
    };

    fn vertical_slots() -> Vec<Rectangle> {
        (0..3)
            .map(|slot_index| {
                Rectangle::new(
                    Point::new(0.0, slot_index as f32 * 20.0),
                    Size::new(100.0, 20.0),
                )
            })
            .collect()
    }

    fn three_slot_glow() -> HoverGlow {
        HoverGlow {
            pager_axis: Axis::Vertical,
            fade_duration: Duration::from_millis(100),
            slot_marks: (0..3)
                .map(|_| HighlightMark::EdgeBand {
                    edge: Edge::Left,
                    corners: 0.0.into(),
                    thickness: 2.0,
                })
                .collect(),
        }
    }

    #[test]
    fn pointing_at_an_unlit_slot_starts_a_fade_until_it_is_aimed() {
        let hover_glow = three_slot_glow();
        let slots = vertical_slots();
        let over_middle = Cursor::Available(Point::new(50.0, 30.0));
        let mut slot_glows = Vec::new();
        assert_eq!(
            hover_glow.pointer_motion(&slot_glows, &slots, over_middle),
            MotionState::Moving
        );
        let start = Instant::now();
        assert_eq!(
            hover_glow.advance(&mut slot_glows, &slots, over_middle, start),
            MotionState::Moving
        );
        assert_eq!(
            hover_glow.pointer_motion(&slot_glows, &slots, over_middle),
            MotionState::Settled
        );
        assert_eq!(
            hover_glow.pointer_motion(&slot_glows, &slots, Cursor::Unavailable),
            MotionState::Moving
        );
    }

    #[test]
    fn a_pointer_outside_every_slot_needs_no_frames() {
        let hover_glow = three_slot_glow();
        let outside = Cursor::Available(Point::new(50.0, 300.0));
        assert_eq!(
            hover_glow.pointer_motion(&Vec::new(), &vertical_slots(), outside),
            MotionState::Settled
        );
    }
}
