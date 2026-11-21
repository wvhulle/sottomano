use std::convert::identity;

use iced::{
    Animation, Color, Rectangle, Theme,
    advanced::{Renderer as _, renderer},
    border::{Border, Radius},
    mouse::Cursor,
    time::{Duration, Instant},
};

use super::{
    edge_band_geometry::{band_clear_of_corners, edge_backdrop, edge_band},
    slot_underlay::{PaintedRow, SlotPainter},
};
use crate::{
    drag_reorder::slot_group::SlotTileInset,
    orientation::{Axis, Edge},
    theming::{ThemedColor, style_metrics::pill_corners},
    transitions::{MotionState, combined_motion, eased_animation},
};

const BACKDROP_TINT_ALPHA: f32 = 0.3;

#[derive(Clone, Copy)]
pub(super) enum HighlightMark {
    Tile {
        tile_inset: SlotTileInset,
    },
    EdgeBand {
        edge: Edge,
        corners: Radius,
        thickness: f32,
    },
}

pub(super) struct HighlightedSlot<'view> {
    pub(super) slot_position: usize,
    pub(super) mark: HighlightMark,
    pub(super) color: ThemedColor<'view>,
}

pub(super) struct TravellingHighlight<'view> {
    pub(super) pager_axis: Axis,
    pub(super) travel_duration: Duration,
    pub(super) highlight_target: HighlightTarget<'view>,
}

pub(super) enum HighlightTarget<'view> {
    Unhighlighted,
    Slot(HighlightedSlot<'view>),
}

#[derive(Clone, Copy)]
struct TravelledSpan {
    start: f32,
    extent: f32,
}

pub(super) struct SpanTravel {
    start: Animation<f32>,
    extent: Animation<f32>,
}

impl SpanTravel {
    fn resting_at(span: TravelledSpan, duration: Duration) -> Self {
        Self {
            start: eased_animation(span.start, duration),
            extent: eased_animation(span.extent, duration),
        }
    }

    fn aim(&mut self, span: TravelledSpan, now: Instant) {
        self.start.go_mut(span.start, now);
        self.extent.go_mut(span.extent, now);
    }

    fn motion_state(&self, now: Instant) -> MotionState {
        combined_motion([&self.start, &self.extent], now)
    }

    fn travelled(&self, now: Instant) -> TravelledSpan {
        TravelledSpan {
            start: self.start.interpolate_with(identity, now),
            extent: self.extent.interpolate_with(identity, now),
        }
    }
}

#[derive(Default)]
pub(super) enum HighlightTravelState {
    #[default]
    Unstarted,
    Travelling {
        span_travel: Box<SpanTravel>,
        frame: Instant,
    },
}

impl TravellingHighlight<'_> {
    fn slot_travelled_span(&self, slot: Rectangle) -> TravelledSpan {
        match self.pager_axis {
            Axis::Vertical => TravelledSpan {
                start: slot.y,
                extent: slot.height,
            },
            Axis::Horizontal => TravelledSpan {
                start: slot.x,
                extent: slot.width,
            },
        }
    }

    fn slot_spanning(&self, slot: Rectangle, travelled: TravelledSpan) -> Rectangle {
        match self.pager_axis {
            Axis::Vertical => Rectangle {
                y: travelled.start,
                height: travelled.extent,
                ..slot
            },
            Axis::Horizontal => Rectangle {
                x: travelled.start,
                width: travelled.extent,
                ..slot
            },
        }
    }

    fn travelled_slot(
        &self,
        painted_row: &PaintedRow<'_, HighlightTravelState>,
    ) -> Option<(&HighlightedSlot<'_>, Rectangle)> {
        let HighlightTarget::Slot(highlighted_slot) = &self.highlight_target else {
            return None;
        };
        let resting_slot = *painted_row
            .child_bounds
            .get(highlighted_slot.slot_position)?;
        let travelled = self.slot_along_travel(resting_slot, painted_row.painter_state);
        Some((highlighted_slot, travelled))
    }

    fn slot_along_travel(
        &self,
        resting_slot: Rectangle,
        travel_state: &HighlightTravelState,
    ) -> Rectangle {
        let HighlightTravelState::Travelling { span_travel, frame } = travel_state else {
            return resting_slot;
        };
        self.slot_spanning(resting_slot, span_travel.travelled(*frame))
    }
}

pub(super) fn fill_slot_mark(
    renderer: &mut iced::Renderer,
    mark: &HighlightMark,
    pager_axis: Axis,
    row_bounds: Rectangle,
    slot: Rectangle,
    color: Color,
) {
    match *mark {
        HighlightMark::Tile { tile_inset } => {
            renderer.fill_quad(tile_inset.tile_on_slot(pager_axis, slot), color);
        }
        HighlightMark::EdgeBand { edge, corners, .. } => renderer.fill_quad(
            renderer::Quad {
                bounds: edge_backdrop(edge, row_bounds, slot),
                border: Border {
                    radius: corners,
                    ..Border::default()
                },
                snap: false,
                ..renderer::Quad::default()
            },
            color.scale_alpha(BACKDROP_TINT_ALPHA),
        ),
    }
}

impl SlotPainter for TravellingHighlight<'_> {
    type PainterState = HighlightTravelState;

    fn advance(
        &self,
        travel_state: &mut HighlightTravelState,
        child_bounds: &[Rectangle],
        _cursor: Cursor,
        now: Instant,
    ) -> MotionState {
        if let HighlightTravelState::Travelling { frame, .. } = travel_state {
            *frame = now;
        }
        let HighlightTarget::Slot(highlighted_slot) = &self.highlight_target else {
            return MotionState::Settled;
        };
        let Some(resting_slot) = child_bounds.get(highlighted_slot.slot_position) else {
            return MotionState::Settled;
        };
        let span = self.slot_travelled_span(*resting_slot);
        match travel_state {
            HighlightTravelState::Travelling { span_travel, .. } => {
                span_travel.aim(span, now);
                span_travel.motion_state(now)
            }
            HighlightTravelState::Unstarted => {
                let mut span_travel = Box::new(SpanTravel::resting_at(span, self.travel_duration));
                span_travel.aim(span, now);
                let motion_state = span_travel.motion_state(now);
                *travel_state = HighlightTravelState::Travelling {
                    span_travel,
                    frame: now,
                };
                motion_state
            }
        }
    }

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, HighlightTravelState>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        let Some((highlighted_slot, travelled)) = self.travelled_slot(&painted_row) else {
            return;
        };
        fill_slot_mark(
            renderer,
            &highlighted_slot.mark,
            self.pager_axis,
            painted_row.row_bounds,
            travelled,
            (highlighted_slot.color)(theme),
        );
    }

    fn draw_over(
        &self,
        painted_row: PaintedRow<'_, HighlightTravelState>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    ) {
        let Some((highlighted_slot, travelled)) = self.travelled_slot(&painted_row) else {
            return;
        };
        let HighlightMark::EdgeBand {
            edge,
            corners,
            thickness,
        } = highlighted_slot.mark
        else {
            return;
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: band_clear_of_corners(
                    edge,
                    edge_band(edge, painted_row.row_bounds, travelled, thickness),
                    corners,
                ),
                border: pill_corners(thickness),
                snap: false,
                ..renderer::Quad::default()
            },
            (highlighted_slot.color)(theme),
        );
    }
}
