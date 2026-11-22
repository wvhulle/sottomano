use iced::{
    advanced::{Layout, Shell, mouse},
    mouse::Cursor,
    time::Instant,
};

use super::{
    NEGLIGIBLE_SCROLL_PX, ScrollOwnership, ScrollSelection, SelectionFollowingScroll, WheelAxes,
    scroll_motion::{OffsetChange, ScrollFrame, ScrollMotion},
};
use crate::{orientation::Axis, pointer_hover::hover_over_bounds, transitions::MotionState};

impl Axis {
    fn wheel_travel(
        self,
        delta: mouse::ScrollDelta,
        line_travel_px: f32,
        wheel_axes: WheelAxes,
    ) -> f32 {
        let (horizontal, vertical, step) = match delta {
            mouse::ScrollDelta::Lines { x, y } => (x, y, line_travel_px),
            mouse::ScrollDelta::Pixels { x, y } => (x, y, 1.0),
        };
        let along_axis = match (self, wheel_axes) {
            (Self::Horizontal, WheelAxes::AlongOnly) => horizontal,
            (Self::Horizontal, WheelAxes::AlongOrVertical) if horizontal != 0.0 => horizontal,
            (Self::Horizontal, WheelAxes::AlongOrVertical) | (Self::Vertical, _) => vertical,
        };
        along_axis * step
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SelectedSpan {
    pub(super) start: f32,
    pub(super) end: f32,
}

impl SelectedSpan {
    pub(super) fn offset_clear_of_fades(
        self,
        offset: f32,
        visible_span: f32,
        fade_extent: f32,
    ) -> f32 {
        if self.start < offset + fade_extent {
            self.start - fade_extent
        } else if self.end > offset + visible_span - fade_extent {
            self.end - visible_span + fade_extent
        } else {
            offset
        }
    }
}

pub(super) struct ScrollerState {
    pub(super) scroll_motion: ScrollMotion,
    pub(super) content_identity: u64,
    pub(super) followed_selection: FollowedSelection,
    pub(super) pointer_button: PointerButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FollowedSelection {
    AwaitingJump,
    FollowedAt(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PointerButton {
    Released,
    Held,
}

impl ScrollerState {
    pub(super) fn content_cursor(&self, axis: Axis, visible_cursor: Cursor) -> Cursor {
        match visible_cursor {
            Cursor::Available(position) if self.pointer_button == PointerButton::Released => {
                Cursor::Available(position + axis.along(self.scroll_motion.hover_drift()))
            }
            Cursor::Available(_) | Cursor::Levitating(_) | Cursor::Unavailable => visible_cursor,
        }
    }
}

pub(super) fn content_layout(layout: Layout<'_>) -> Layout<'_> {
    layout.children().next().unwrap()
}

impl<Message> SelectionFollowingScroll<'_, Message> {
    fn max_offset(&self, layout: Layout<'_>) -> f32 {
        let content_span = self.axis.size_span(content_layout(layout).bounds().size());
        (content_span - self.axis.size_span(layout.bounds().size())).max(0.0)
    }

    fn scroll_to(&self, state: &mut ScrollerState, offset: f32, shell: &mut Shell<'_, Message>) {
        match &self.ownership {
            ScrollOwnership::SelfOwned => state.scroll_motion.set_target(offset),
            ScrollOwnership::Controlled { on_scroll, .. } => shell.publish(on_scroll(offset)),
        }
        shell.request_redraw();
    }

    pub(super) fn advance_scroll(
        &self,
        state: &mut ScrollerState,
        layout: Layout<'_>,
        cursor: Cursor,
        now: Instant,
        shell: &mut Shell<'_, Message>,
    ) {
        let offset_change = state.scroll_motion.advance(ScrollFrame {
            now,
            max_offset: self.max_offset(layout),
            pointer_hover: hover_over_bounds(cursor, layout.bounds()),
        });
        if offset_change == OffsetChange::Moved {
            shell.invalidate_layout();
        }
        if state.scroll_motion.travel_state(now) == MotionState::Moving {
            shell.request_redraw();
        }
    }

    pub(super) fn keep_selection_clear_of_fades(
        &self,
        state: &mut ScrollerState,
        layout: Layout<'_>,
        shell: &mut Shell<'_, Message>,
    ) {
        let ScrollSelection::SelectedAt(selected_position) = self.selection else {
            return;
        };
        let previously_followed = state.followed_selection;
        if previously_followed == FollowedSelection::FollowedAt(selected_position) {
            return;
        }
        state.followed_selection = FollowedSelection::FollowedAt(selected_position);
        let max_offset = self.max_offset(layout);
        let content = content_layout(layout);
        let Some(selected_node) = content.children().nth(selected_position) else {
            return;
        };
        if max_offset <= 0.0 {
            return;
        }
        let selected_bounds = selected_node.bounds();
        let start = self.axis.point_coordinate(selected_bounds.position())
            - self.axis.point_coordinate(content.bounds().position());
        let visible_span = self.axis.size_span(layout.bounds().size());
        let current = state.scroll_motion.target_offset;
        let target = SelectedSpan {
            start,
            end: start + self.axis.size_span(selected_bounds.size()),
        }
        .offset_clear_of_fades(
            current,
            visible_span,
            self.edge_fade.extent_within(visible_span),
        )
        .clamp(0.0, max_offset);
        if (target - current).abs() <= NEGLIGIBLE_SCROLL_PX {
            return;
        }
        if previously_followed == FollowedSelection::AwaitingJump {
            state.scroll_motion.jump_to(target);
            shell.invalidate_layout();
        }
        self.scroll_to(state, target, shell);
    }

    pub(super) fn scroll_by_wheel(
        &self,
        state: &mut ScrollerState,
        layout: Layout<'_>,
        delta: mouse::ScrollDelta,
        shell: &mut Shell<'_, Message>,
    ) {
        let current = state.scroll_motion.target_offset;
        let next = (current
            - self
                .axis
                .wheel_travel(delta, self.wheel_line_travel_px, self.wheel_axes))
        .clamp(0.0, self.max_offset(layout));
        if (next - current).abs() <= f32::EPSILON {
            return;
        }
        if matches!(delta, mouse::ScrollDelta::Pixels { .. }) {
            state.scroll_motion.jump_to(next);
            shell.invalidate_layout();
        }
        self.scroll_to(state, next, shell);
        shell.capture_event();
    }
}
