mod timeline_layout;
mod vertical_scale;

use std::time::Duration;

use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    advanced::svg::Svg,
    alignment::Vertical,
    mouse::Cursor,
    widget::{
        canvas,
        canvas::{LineCap, Stroke},
        svg,
        text::Alignment,
    },
};

pub use self::timeline_layout::{
    MarkShape, RailPresence, SpanInk, SpanWeight, TimelineAxis, TimelineMark, TimelineScale,
    TimelineSpan,
};
use self::{
    timeline_layout::{
        PixelRun, center_inside, estimated_text_width, glyphs_merged_by_time, marks_in_time_order,
        runs_around_gaps,
    },
    vertical_scale::{VerticalMetrics, VerticalScale},
};
use crate::theming::{
    LINE_HEIGHT, TextFace, background_strong_color, danger_color, primary_color,
    secondary_text_color, text_color, warning_color,
};

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
    pub fn view<'a, Message: 'a>(self) -> Element<'a, Message> {
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

    fn column_layout(&self) -> ColumnLayout {
        let size_px = self.label_face.size_px;
        let widest_text = |shape_filter: fn(&MarkShape) -> bool,
                           text_of: fn(&TimelineMark) -> String| {
            self.marks
                .iter()
                .filter(|mark| shape_filter(&mark.shape))
                .map(|mark| estimated_text_width(&text_of(mark), size_px))
                .fold(0.0, f32::max)
        };
        let is_event = |shape: &MarkShape| *shape == MarkShape::PointEvent;
        let is_interval = |shape: &MarkShape| matches!(shape, MarkShape::Interval { .. });
        let time_column_width = widest_text(is_event, |mark| mark.time_text.clone());
        let event_detail_x = time_column_width + COLUMN_GAP_PX;
        let ring_extent = size_px * EVENT_RING_SCALE;
        let rail_x = event_detail_x
            + widest_text(is_event, |mark| mark.detail_text.clone())
            + COLUMN_GAP_PX
            + ring_extent / 2.0;
        let interval_glyph_x = rail_x + ring_extent / 2.0 + COLUMN_GAP_PX;
        let widest_interval_glyph_run = self
            .marks
            .iter()
            .filter(|mark| is_interval(&mark.shape))
            .map(|mark| self.glyph_extent(mark.glyph_svgs.len()))
            .fold(0.0, f32::max);
        let interval_text_x = if widest_interval_glyph_run > 0.0 {
            interval_glyph_x + widest_interval_glyph_run + GLYPH_GAP_PX * 2.0
        } else {
            interval_glyph_x
        };
        ColumnLayout {
            time_column_width,
            event_detail_x,
            rail_x,
            interval_glyph_x,
            interval_text_x,
            natural_width: interval_text_x + widest_text(is_interval, interval_label_text),
        }
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

    fn draw_labeled_column(&self, frame: &mut canvas::Frame, theme: &Theme) {
        let scale = self.vertical_scale();
        let size_px = self.label_face.size_px;
        let ring_radius = size_px * EVENT_RING_SCALE / 2.0;
        let ColumnLayout {
            time_column_width,
            event_detail_x,
            rail_x,
            interval_glyph_x,
            interval_text_x,
            ..
        } = self.column_layout();
        let ring_runs: Vec<PixelRun> = self
            .marks
            .iter()
            .zip(&scale.mark_ys)
            .filter(|(mark, _)| mark.shape == MarkShape::PointEvent && !mark.glyph_svgs.is_empty())
            .map(|(_, mark_y)| PixelRun {
                start: mark_y - ring_radius,
                end: mark_y + ring_radius,
            })
            .chain(scale.compressed_gap_ys.iter().map(|gap_y| PixelRun {
                start: gap_y - COMPRESSION_TICK_PX,
                end: gap_y + COMPRESSION_TICK_PX,
            }))
            .collect();
        let rail = RailGeometry {
            orientation: RailOrientation::Vertical { rail_x },
            whole_run: PixelRun {
                start: scale.top_y(),
                end: scale.bottom_y(),
            },
            gaps: padded_gaps(&ring_runs),
        };
        self.draw_spans_along(frame, theme, &rail, |offset| scale.y_of(offset));
        for gap_y in &scale.compressed_gap_ys {
            for tick_offset in [-COMPRESSION_TICK_PX, COMPRESSION_TICK_PX] {
                frame.stroke(
                    &canvas::Path::line(
                        Point::new(rail_x - COMPRESSION_TICK_PX, gap_y + tick_offset + 2.0),
                        Point::new(rail_x + COMPRESSION_TICK_PX, gap_y + tick_offset - 2.0),
                    ),
                    rail_stroke(secondary_text_color(theme), THIN_STROKE_PX / 2.0),
                );
            }
        }
        for (mark, mark_y) in self.marks.iter().zip(&scale.mark_ys) {
            let mark_color = ink_color(mark.ink, theme);
            match mark.shape {
                MarkShape::PointEvent => {
                    self.draw_event_ring(frame, mark, Point::new(rail_x, *mark_y), mark_color);
                    frame.fill_text(self.label_text(
                        &mark.time_text,
                        Point::new(time_column_width, *mark_y),
                        Alignment::Right,
                        secondary_text_color(theme),
                    ));
                    frame.fill_text(self.label_text(
                        &mark.detail_text,
                        Point::new(event_detail_x, *mark_y),
                        Alignment::Left,
                        text_color(theme),
                    ));
                }
                MarkShape::Interval { .. } => {
                    let faint_color = secondary_text_color(theme);
                    for (glyph_index, glyph_svg) in mark.glyph_svgs.iter().enumerate() {
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "a mark joins a handful of glyphs"
                        )]
                        let glyph_center_x = interval_glyph_x
                            + size_px / 2.0
                            + glyph_index as f32 * (size_px + GLYPH_GAP_PX);
                        self.draw_glyph(
                            frame,
                            glyph_svg,
                            Point::new(glyph_center_x, *mark_y),
                            faint_color,
                        );
                    }
                    frame.fill_text(self.label_text(
                        &interval_label_text(mark),
                        Point::new(interval_text_x, *mark_y),
                        Alignment::Left,
                        faint_color,
                    ));
                }
            }
        }
    }

    fn draw_event_ring(
        &self,
        frame: &mut canvas::Frame,
        mark: &TimelineMark,
        center: Point,
        color: Color,
    ) {
        let Some(first_glyph) = mark.glyph_svgs.first() else {
            return;
        };
        let size_px = self.label_face.size_px;
        frame.stroke(
            &canvas::Path::circle(
                center,
                size_px * EVENT_RING_SCALE / 2.0 - THIN_STROKE_PX / 2.0,
            ),
            Stroke::default()
                .with_color(color)
                .with_width(THIN_STROKE_PX / 1.5),
        );
        draw_glyph_sized(
            frame,
            first_glyph,
            center,
            color,
            size_px * EVENT_GLYPH_SCALE,
        );
    }

    fn label_text(
        &self,
        content: &str,
        anchor: Point,
        align_x: Alignment,
        color: Color,
    ) -> canvas::Text {
        canvas::Text {
            content: content.to_owned(),
            position: anchor,
            color,
            size: self.label_face.size_px.into(),
            line_height: LINE_HEIGHT,
            font: self.label_face.font,
            align_x,
            align_y: Vertical::Center,
            ..canvas::Text::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ColumnLayout {
    time_column_width: f32,
    event_detail_x: f32,
    rail_x: f32,
    interval_glyph_x: f32,
    interval_text_x: f32,
    natural_width: f32,
}

#[derive(Debug, Clone, Copy)]
enum RailOrientation {
    Horizontal { rail_y: f32 },
    Vertical { rail_x: f32 },
}

struct RailGeometry {
    orientation: RailOrientation,
    whole_run: PixelRun,
    gaps: Vec<PixelRun>,
}

impl RailGeometry {
    fn point_at(&self, along: f32) -> Point {
        match self.orientation {
            RailOrientation::Horizontal { rail_y } => Point::new(along, rail_y),
            RailOrientation::Vertical { rail_x } => Point::new(rail_x, along),
        }
    }

    fn stroke_gapped(&self, frame: &mut canvas::Frame, run: PixelRun, stroke: Stroke<'static>) {
        let ascending_run = PixelRun {
            start: run.start.min(run.end),
            end: run.start.max(run.end),
        };
        for visible_run in runs_around_gaps(ascending_run, &self.gaps) {
            frame.stroke(
                &canvas::Path::line(
                    self.point_at(visible_run.start),
                    self.point_at(visible_run.end),
                ),
                stroke,
            );
        }
    }
}

fn draw_glyph_sized(
    frame: &mut canvas::Frame,
    glyph_svg: &'static [u8],
    center: Point,
    color: Color,
    size_px: f32,
) {
    frame.draw_svg(
        Rectangle::new(
            Point::new(center.x - size_px / 2.0, center.y - size_px / 2.0),
            Size::new(size_px, size_px),
        ),
        Svg::new(svg::Handle::from_memory(glyph_svg)).color(color),
    );
}

fn interval_label_text(mark: &TimelineMark) -> String {
    mark.detail_text.clone()
}

fn padded_gaps(glyph_runs: &[PixelRun]) -> Vec<PixelRun> {
    glyph_runs
        .iter()
        .map(|glyph_run| PixelRun {
            start: glyph_run.start - GLYPH_CLEARANCE_PX,
            end: glyph_run.end + GLYPH_CLEARANCE_PX,
        })
        .collect()
}

fn rail_stroke(color: Color, width: f32) -> Stroke<'static> {
    Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(LineCap::Butt)
}

fn ink_color(ink: SpanInk, theme: &Theme) -> Color {
    match ink {
        SpanInk::Faint => background_strong_color(theme),
        SpanInk::Strong => secondary_text_color(theme),
        SpanInk::Accent => primary_color(theme),
        SpanInk::Warning => warning_color(theme),
        SpanInk::Critical => danger_color(theme),
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
