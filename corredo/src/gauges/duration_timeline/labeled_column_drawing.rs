use iced::{
    Color, Point, Theme,
    alignment::Vertical,
    widget::{canvas, canvas::Stroke, text::Alignment},
};

use super::{
    COLUMN_GAP_PX, COMPRESSION_TICK_PX, DurationTimeline, EVENT_GLYPH_SCALE, EVENT_RING_SCALE,
    GLYPH_GAP_PX, MarkMeasure, MarkShape, THIN_STROKE_PX, TimelineMark,
    gapped_rail_strokes::{
        RailGeometry, RailOrientation, draw_glyph_sized, ink_color, padded_gaps, rail_stroke,
    },
    timeline_layout::{PixelRun, estimated_text_width},
};
use crate::theming::{LINE_HEIGHT, secondary_text_color, text_color};

impl DurationTimeline {
    pub(super) fn column_layout(&self) -> ColumnLayout {
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
        let event_measure_x =
            event_detail_x + widest_text(is_event, |mark| mark.detail_text.clone());
        let widest_measure = widest_text(is_event, |mark| mark.measure.measure_text().to_owned());
        let event_measure_x = if widest_measure > 0.0 {
            event_measure_x + COLUMN_GAP_PX
        } else {
            event_measure_x
        };
        let rail_x = event_measure_x + widest_measure + COLUMN_GAP_PX + ring_extent / 2.0;
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
            event_measure_x,
            rail_x,
            interval_glyph_x,
            interval_text_x,
            natural_width: interval_text_x + widest_text(is_interval, interval_label_text),
        }
    }

    pub(super) fn draw_labeled_column(&self, frame: &mut canvas::Frame, theme: &Theme) {
        let scale = self.vertical_scale();
        let size_px = self.label_face.size_px;
        let ring_radius = size_px * EVENT_RING_SCALE / 2.0;
        let ColumnLayout {
            time_column_width,
            event_detail_x,
            event_measure_x,
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
        self.draw_spans_along(frame, theme, &rail, |offset| {
            scale.vertical_position_at(offset)
        });
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
                    if let MarkMeasure::Measured(measure_text) = &mark.measure {
                        frame.fill_text(self.label_text(
                            measure_text,
                            Point::new(event_measure_x, *mark_y),
                            Alignment::Left,
                            secondary_text_color(theme),
                        ));
                    }
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
pub(super) struct ColumnLayout {
    pub(super) time_column_width: f32,
    pub(super) event_detail_x: f32,
    pub(super) event_measure_x: f32,
    pub(super) rail_x: f32,
    pub(super) interval_glyph_x: f32,
    pub(super) interval_text_x: f32,
    pub(super) natural_width: f32,
}

fn interval_label_text(mark: &TimelineMark) -> String {
    mark.detail_text.clone()
}
