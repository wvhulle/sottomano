use std::convert::identity;

use iced::{
    Animation, Rectangle, Theme,
    advanced::{Renderer as _, renderer},
    border::{Border, Radius},
    time::{Duration, Instant},
};

use super::{
    PagerEdge,
    edge_band_geometry::{band_clear_of_corners, edge_backdrop, edge_band},
    sliding_row::eased_motion,
    slot_underlay::{PaintedRow, SlotPainter},
};
use crate::{
    pointer_drag::slot_group::{PagerAxis, SlotTileInset, TileNeighbours},
    primitives::motion_state::MotionState,
    shared_metrics::pill_corners,
    theme_colors::ThemedColor,
};

const BACKDROP_TINT_ALPHA: f32 = 0.3;

pub(super) enum HighlightMark {
    Tile {
        tile_inset: SlotTileInset,
        tile_neighbours: TileNeighbours,
    },
    EdgeBand {
        edge: PagerEdge,
        corners: Radius,
        thickness: f32,
    },
}

pub(super) struct HighlightedSlot<'a> {
    pub(super) slot_position: usize,
    pub(super) mark: HighlightMark,
    pub(super) color: ThemedColor<'a>,
}

pub(super) struct TravellingHighlight<'a> {
    pub(super) pager_axis: PagerAxis,
    pub(super) travel_duration: Duration,
    pub(super) highlighted_slot: Option<HighlightedSlot<'a>>,
}

#[derive(Clone, Copy)]
struct TravelledSpan {
    start: f32,
    extent: f32,
}

struct SpanTravel {
    start: Animation<f32>,
    extent: Animation<f32>,
}

impl SpanTravel {
    fn resting_at(span: TravelledSpan, duration: Duration) -> Self {
        Self {
            start: eased_motion(span.start, duration),
            extent: eased_motion(span.extent, duration),
        }
    }

    fn aim(&mut self, span: TravelledSpan, now: Instant) {
        self.start.go_mut(span.start, now);
        self.extent.go_mut(span.extent, now);
    }

    fn motion_state(&self, now: Instant) -> MotionState {
        if self.start.is_animating(now) || self.extent.is_animating(now) {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }

    fn travelled(&self, now: Instant) -> TravelledSpan {
        TravelledSpan {
            start: self.start.interpolate_with(identity, now),
            extent: self.extent.interpolate_with(identity, now),
        }
    }
}

#[derive(Default)]
pub(super) struct HighlightTravelState {
    span_travel: Option<SpanTravel>,
    frame: Option<Instant>,
}

impl TravellingHighlight<'_> {
    fn span_of(&self, slot: Rectangle) -> TravelledSpan {
        match self.pager_axis {
            PagerAxis::Vertical => TravelledSpan {
                start: slot.y,
                extent: slot.height,
            },
            PagerAxis::Horizontal => TravelledSpan {
                start: slot.x,
                extent: slot.width,
            },
        }
    }

    fn slot_spanning(&self, slot: Rectangle, travelled: TravelledSpan) -> Rectangle {
        match self.pager_axis {
            PagerAxis::Vertical => Rectangle {
                y: travelled.start,
                height: travelled.extent,
                ..slot
            },
            PagerAxis::Horizontal => Rectangle {
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
        let highlighted_slot = self.highlighted_slot.as_ref()?;
        let resting_slot = *painted_row
            .child_bounds
            .get(highlighted_slot.slot_position)?;
        let travel_state = painted_row.painter_state;
        let travelled = match (&travel_state.span_travel, travel_state.frame) {
            (Some(span_travel), Some(frame)) => {
                self.slot_spanning(resting_slot, span_travel.travelled(frame))
            }
            _ => resting_slot,
        };
        Some((highlighted_slot, travelled))
    }
}

impl SlotPainter for TravellingHighlight<'_> {
    type PainterState = HighlightTravelState;

    fn advance(
        &self,
        travel_state: &mut HighlightTravelState,
        child_bounds: &[Rectangle],
        now: Instant,
    ) -> MotionState {
        travel_state.frame = Some(now);
        let Some(resting_slot) = self
            .highlighted_slot
            .as_ref()
            .and_then(|highlighted_slot| child_bounds.get(highlighted_slot.slot_position))
        else {
            return MotionState::Settled;
        };
        let span = self.span_of(*resting_slot);
        let span_travel = travel_state
            .span_travel
            .get_or_insert_with(|| SpanTravel::resting_at(span, self.travel_duration));
        span_travel.aim(span, now);
        span_travel.motion_state(now)
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
        let color = (highlighted_slot.color)(theme);
        match highlighted_slot.mark {
            HighlightMark::Tile {
                tile_inset,
                tile_neighbours,
            } => renderer.fill_quad(
                tile_inset.tile_on_slot(self.pager_axis, travelled, tile_neighbours),
                color,
            ),
            HighlightMark::EdgeBand { edge, corners, .. } => renderer.fill_quad(
                renderer::Quad {
                    bounds: edge_backdrop(edge, painted_row.row_bounds, travelled),
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
