mod gapped_rail_strokes;
mod labeled_column_drawing;
mod timeline_layout;
mod vertical_scale;

use std::time::Duration;

use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse::Cursor, widget::canvas,
};

pub use self::timeline_layout::{
    MarkMeasure, MarkShape, RailPresence, SpanInk, SpanWeight, TimelineAxis, TimelineMark,
    TimelineScale, TimelineSpan,
};
use self::{
    gapped_rail_strokes::{
        RailGeometry, RailOrientation, draw_glyph_sized, ink_color, padded_gaps, rail_stroke,
    },
    timeline_layout::{PixelRun, center_inside, glyphs_merged_by_time, marks_in_time_order},
    vertical_scale::{VerticalMetrics, VerticalScale},
};
use crate::theming::{TextFace, background_strong_color, text_color};

const COMPACT_TEXT_ROWS: f32 = 1.2;
const THIN_STROKE_PX: f32 = 2.5;
const THICK_STROKE_PX: f32 = 6.0;
const NOW_DOT_RADIUS_PX: f32 = 4.0;
const GLYPH_CLEARANCE_PX: f32 = 3.0;
const LINE_HEIGHT_FACTOR: f32 = 1.4;
const GLYPH_GAP_PX: f32 = 2.0;
const COLUMN_GAP_PX: f32 = 8.0;
const MINUTES_PER_ROW: f32 = 10.0;
const LONGEST_GAP_ROWS: f32 = 4.0;
const MARK_SPACING_ROWS: f32 = 1.6;
const COMPRESSION_TICK_PX: f32 = 4.0;
const EVENT_RING_SCALE: f32 = 1.5;
const EVENT_GLYPH_SCALE: f32 = 0.7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NowCursor {
    Shown(Duration),
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineDirection {
    OldestFirst,
    NewestFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineDensity {
    CompactRail,
    LabeledColumn,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DurationTimeline {
    axis: TimelineAxis,
    spans: Vec<TimelineSpan>,
    marks: Vec<TimelineMark>,
    now_cursor: NowCursor,
    density: TimelineDensity,
    direction: TimelineDirection,
    label_face: TextFace,
}

impl DurationTimeline {
    #[must_use]
    pub fn new(axis: TimelineAxis, label_face: TextFace) -> Self {
        Self {
            axis,
            spans: Vec::new(),
            marks: Vec::new(),
            now_cursor: NowCursor::Hidden,
            density: TimelineDensity::LabeledColumn,
            direction: TimelineDirection::OldestFirst,
            label_face,
        }
    }

    #[must_use]
    pub fn spans(self, spans: Vec<TimelineSpan>) -> Self {
        Self { spans, ..self }
    }

    #[must_use]
    pub fn marks(self, marks: Vec<TimelineMark>) -> Self {
        Self {
            marks: marks_in_time_order(marks),
            ..self
        }
    }

    #[must_use]
    pub fn now_cursor(self, now_cursor: NowCursor) -> Self {
        Self { now_cursor, ..self }
    }

    #[must_use]
    pub fn direction(self, direction: TimelineDirection) -> Self {
        Self { direction, ..self }
    }

    #[must_use]
    pub fn density(self, density: TimelineDensity) -> Self {
        Self { density, ..self }
    }

    #[must_use]
    pub fn height(&self) -> f32 {
        match self.density {
            TimelineDensity::CompactRail => COMPACT_TEXT_ROWS * self.row_height(),
            TimelineDensity::LabeledColumn => self.vertical_scale().total_height,
        }
    }

    #[must_use]
    pub fn view<'view, Message: 'view>(self) -> Element<'view, Message> {
        let height = self.height();
        let width = match self.density {
            TimelineDensity::CompactRail => Length::Fill,
            TimelineDensity::LabeledColumn => Length::Fixed(self.column_layout().natural_width),
        };
        canvas(self)
            .width(width)
            .height(Length::Fixed(height))
            .into()
    }

    fn row_height(&self) -> f32 {
        self.label_face.size_px * LINE_HEIGHT_FACTOR
    }

    fn glyph_extent(&self, glyph_count: usize) -> f32 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a mark joins a handful of glyphs"
        )]
        let glyph_count = glyph_count as f32;
        let size_px = self.label_face.size_px;
        (glyph_count * (size_px + GLYPH_GAP_PX) - GLYPH_GAP_PX).max(0.0)
    }

    fn vertical_scale(&self) -> VerticalScale {
        let row_height_px = self.row_height();
        let mark_times: Vec<Duration> = self.marks.iter().map(TimelineMark::label_anchor).collect();
        let oldest_first = VerticalScale::laid_out(
            &mark_times,
            self.axis.span_end,
            VerticalMetrics {
                row_height_px,
                mark_spacing_px: row_height_px * MARK_SPACING_ROWS,
                pixels_per_minute: row_height_px / MINUTES_PER_ROW,
                longest_gap_px: row_height_px * LONGEST_GAP_ROWS,
            },
        );
        match self.direction {
            TimelineDirection::OldestFirst => oldest_first,
            TimelineDirection::NewestFirst => oldest_first.newest_first(),
        }
    }

    fn draw_glyph(
        &self,
        frame: &mut canvas::Frame,
        glyph_svg: &'static [u8],
        center: Point,
        color: Color,
    ) {
        draw_glyph_sized(frame, glyph_svg, center, color, self.label_face.size_px);
    }

    fn draw_spans_along(
        &self,
        frame: &mut canvas::Frame,
        theme: &Theme,
        rail: &RailGeometry,
        position_of: impl Fn(Duration) -> f32,
    ) {
        rail.stroke_gapped(
            frame,
            rail.whole_run,
            rail_stroke(background_strong_color(theme), THIN_STROKE_PX),
        );
        for span in &self.spans {
            let stroke_width = match span.weight {
                SpanWeight::Thin => THIN_STROKE_PX,
                SpanWeight::Thick => THICK_STROKE_PX,
            };
            rail.stroke_gapped(
                frame,
                PixelRun {
                    start: position_of(span.from),
                    end: position_of(span.until),
                },
                rail_stroke(ink_color(span.ink, theme), stroke_width),
            );
        }
        if let NowCursor::Shown(now_offset) = self.now_cursor {
            frame.fill(
                &canvas::Path::circle(rail.point_at(position_of(now_offset)), NOW_DOT_RADIUS_PX),
                text_color(theme),
            );
        }
    }

    fn draw_compact_rail(&self, frame: &mut canvas::Frame, theme: &Theme, bounds: Size) {
        let rail_y = bounds.height / 2.0;
        let x_of = |offset: Duration| self.axis.axis_fraction(offset) * bounds.width;
        let merged_marks = glyphs_merged_by_time(&self.marks);
        let glyph_runs: Vec<PixelRun> = merged_marks
            .iter()
            .map(|mark| {
                let extent = self.glyph_extent(mark.glyph_svgs.len());
                let center_x = center_inside(x_of(mark.at), extent, bounds.width);
                PixelRun {
                    start: center_x - extent / 2.0,
                    end: center_x + extent / 2.0,
                }
            })
            .collect();
        let rail = RailGeometry {
            orientation: RailOrientation::Horizontal { rail_y },
            whole_run: PixelRun {
                start: 0.0,
                end: bounds.width,
            },
            gaps: padded_gaps(&glyph_runs),
        };
        self.draw_spans_along(frame, theme, &rail, x_of);
        let size_px = self.label_face.size_px;
        for (mark, glyph_run) in merged_marks.iter().zip(&glyph_runs) {
            for (glyph_index, glyph_svg) in mark.glyph_svgs.iter().enumerate() {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a mark joins a handful of glyphs"
                )]
                let glyph_center_x =
                    glyph_run.start + size_px / 2.0 + glyph_index as f32 * (size_px + GLYPH_GAP_PX);
                self.draw_glyph(
                    frame,
                    glyph_svg,
                    Point::new(glyph_center_x, rail_y),
                    ink_color(mark.ink, theme),
                );
            }
        }
    }
}

impl<Message> canvas::Program<Message> for DurationTimeline {
    type State = ();

    fn draw(
        &self,
        (): &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        match self.density {
            TimelineDensity::CompactRail => {
                self.draw_compact_rail(&mut frame, theme, bounds.size());
            }
            TimelineDensity::LabeledColumn => self.draw_labeled_column(&mut frame, theme),
        }
        vec![frame.into_geometry()]
    }
}
