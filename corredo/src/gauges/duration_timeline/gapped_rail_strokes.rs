use iced::{
    Color, Point, Rectangle, Size, Theme,
    advanced::svg::Svg,
    widget::{
        canvas,
        canvas::{LineCap, Stroke},
        svg,
    },
};

use super::{
    GLYPH_CLEARANCE_PX, SpanInk,
    timeline_layout::{PixelRun, runs_around_gaps},
};
use crate::theming::{
    background_strong_color, danger_color, primary_color, secondary_text_color, warning_color,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum RailOrientation {
    Horizontal { rail_y: f32 },
    Vertical { rail_x: f32 },
}

pub(super) struct RailGeometry {
    pub(super) orientation: RailOrientation,
    pub(super) whole_run: PixelRun,
    pub(super) gaps: Vec<PixelRun>,
}

impl RailGeometry {
    pub(super) fn point_at(&self, along: f32) -> Point {
        match self.orientation {
            RailOrientation::Horizontal { rail_y } => Point::new(along, rail_y),
            RailOrientation::Vertical { rail_x } => Point::new(rail_x, along),
        }
    }

    pub(super) fn stroke_gapped(
        &self,
        frame: &mut canvas::Frame,
        run: PixelRun,
        stroke: Stroke<'static>,
    ) {
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

pub(super) fn draw_glyph_sized(
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

pub(super) fn padded_gaps(glyph_runs: &[PixelRun]) -> Vec<PixelRun> {
    glyph_runs
        .iter()
        .map(|glyph_run| PixelRun {
            start: glyph_run.start - GLYPH_CLEARANCE_PX,
            end: glyph_run.end + GLYPH_CLEARANCE_PX,
        })
        .collect()
}

pub(super) fn rail_stroke(color: Color, width: f32) -> Stroke<'static> {
    Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(LineCap::Butt)
}

pub(super) fn ink_color(ink: SpanInk, theme: &Theme) -> Color {
    match ink {
        SpanInk::Faint => background_strong_color(theme),
        SpanInk::Strong => secondary_text_color(theme),
        SpanInk::Accent => primary_color(theme),
        SpanInk::Warning => warning_color(theme),
        SpanInk::Critical => danger_color(theme),
    }
}
