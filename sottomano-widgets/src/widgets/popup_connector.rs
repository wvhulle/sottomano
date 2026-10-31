use iced::{
    Border, Color, Point, Rectangle, Shadow, Size, Theme, Vector,
    advanced::{Renderer as _, graphics::geometry::Renderer as _, renderer},
    border::Radius,
    widget::canvas::{Frame, Path, Stroke, path::Builder},
};
use sottomano_config::screen_edges::Position;

use crate::widgets::panel_frame::LayerFrameStyle;

const BASE_HALF_WIDTH_PX: f32 = 6.0;
const SHOULDER_PX: f32 = BASE_HALF_WIDTH_PX;

#[derive(Debug, Clone, Copy)]
pub struct FramedSurface {
    pub frame: LayerFrameStyle,
    pub bounds: Rectangle,
    pub corners: Radius,
    pub shadow: Shadow,
}

pub struct ConnectorTriangle {
    position: Position,
    apex: Point,
    base_start: Point,
    base_end: Point,
}

fn centered_within(target: f32, start: f32, end: f32, inset: f32) -> f32 {
    let first = start + inset;
    let last = end - inset;
    if first <= last {
        target.clamp(first, last)
    } else {
        f32::midpoint(start, end)
    }
}

fn toward(from: Point, to: Point, fraction: f32) -> Point {
    from + (to - from) * fraction
}

impl ConnectorTriangle {
    #[must_use]
    pub const fn half_span() -> f32 {
        BASE_HALF_WIDTH_PX + SHOULDER_PX
    }

    #[must_use]
    pub fn between(
        anchor: Rectangle,
        bar_edge: f32,
        popup: Rectangle,
        position: Position,
        corner_radius: f32,
    ) -> Self {
        let inset = Self::half_span() + corner_radius;
        match position {
            Position::Top | Position::Bottom => {
                let base_y = if position == Position::Top {
                    popup.y
                } else {
                    popup.y + popup.height
                };
                let base_center =
                    centered_within(anchor.center_x(), popup.x, popup.x + popup.width, inset);
                Self {
                    position,
                    apex: Point::new(anchor.center_x(), bar_edge),
                    base_start: Point::new(base_center - BASE_HALF_WIDTH_PX, base_y),
                    base_end: Point::new(base_center + BASE_HALF_WIDTH_PX, base_y),
                }
            }
            Position::Left => {
                let base_center =
                    centered_within(anchor.center_y(), popup.y, popup.y + popup.height, inset);
                Self {
                    position,
                    apex: Point::new(bar_edge, anchor.center_y()),
                    base_start: Point::new(popup.x, base_center - BASE_HALF_WIDTH_PX),
                    base_end: Point::new(popup.x, base_center + BASE_HALF_WIDTH_PX),
                }
            }
        }
    }

    #[must_use]
    pub fn translated(&self, offset: Vector) -> Self {
        Self {
            position: self.position,
            apex: self.apex + offset,
            base_start: self.base_start + offset,
            base_end: self.base_end + offset,
        }
    }

    fn shoulders(&self) -> [Point; 2] {
        let base = self.base_end - self.base_start;
        let length = base.x.hypot(base.y).max(f32::EPSILON);
        let along = base * (SHOULDER_PX / length);
        [self.base_start - along, self.base_end + along]
    }

    fn bounds(&self) -> Rectangle {
        let [shoulder_start, shoulder_end] = self.shoulders();
        let corners = [self.apex, shoulder_start, shoulder_end];
        let left = corners
            .iter()
            .map(|corner| corner.x)
            .fold(f32::INFINITY, f32::min);
        let top = corners
            .iter()
            .map(|corner| corner.y)
            .fold(f32::INFINITY, f32::min);
        let right = corners
            .iter()
            .map(|corner| corner.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let bottom = corners
            .iter()
            .map(|corner| corner.y)
            .fold(f32::NEG_INFINITY, f32::max);
        Rectangle::new(Point::new(left, top), Size::new(right - left, bottom - top))
    }

    fn depth(&self) -> f32 {
        match self.position {
            Position::Top | Position::Bottom => (self.base_start.y - self.apex.y).abs(),
            Position::Left => (self.base_start.x - self.apex.x).abs(),
        }
    }

    fn cross_section(&self, from_apex: f32) -> [Point; 2] {
        let fraction = (from_apex / self.depth()).min(1.0);
        [
            toward(self.apex, self.base_start, fraction),
            toward(self.apex, self.base_end, fraction),
        ]
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    #[must_use]
    pub fn blur_rows(&self) -> Vec<Rectangle> {
        let bounds = self.bounds();
        match self.position {
            Position::Top | Position::Bottom => {
                let top = bounds.y.ceil() as i32;
                let bottom = (bounds.y + bounds.height).floor() as i32;
                (top..bottom)
                    .map(|row| {
                        let from_apex = if self.position == Position::Top {
                            row as f32 + 1.0 - self.apex.y
                        } else {
                            self.apex.y - row as f32
                        };
                        let [start, end] = self.cross_section(from_apex);
                        Rectangle::new(
                            Point::new(start.x, row as f32),
                            Size::new(end.x - start.x, 1.0),
                        )
                    })
                    .collect()
            }
            Position::Left => {
                let left = bounds.x.ceil() as i32;
                let right = (bounds.x + bounds.width).floor() as i32;
                (left..right)
                    .map(|column| {
                        let [start, end] = self.cross_section(column as f32 + 1.0 - self.apex.x);
                        Rectangle::new(
                            Point::new(column as f32, start.y),
                            Size::new(1.0, end.y - start.y),
                        )
                    })
                    .collect()
            }
        }
    }

    fn rounded_tip_share(&self) -> f32 {
        let flank = self.base_start - self.apex;
        (SHOULDER_PX / flank.x.hypot(flank.y).max(f32::EPSILON)).min(0.5)
    }

    fn trace_sides(&self, path: &mut Builder, reversed: bool) {
        let [shoulder_start, shoulder_end] = self.shoulders();
        let (near_base, far_base, near_shoulder, far_shoulder) = if reversed {
            (self.base_end, self.base_start, shoulder_end, shoulder_start)
        } else {
            (self.base_start, self.base_end, shoulder_start, shoulder_end)
        };
        let tip_share = self.rounded_tip_share();
        path.line_to(near_shoulder);
        path.bezier_curve_to(
            near_base,
            toward(near_base, self.apex, 0.5),
            toward(self.apex, near_base, tip_share),
        );
        path.quadratic_curve_to(self.apex, toward(self.apex, far_base, tip_share));
        path.bezier_curve_to(toward(far_base, self.apex, 0.5), far_base, far_shoulder);
    }

    fn joined_outline(&self, panel: Rectangle, corners: Radius) -> Path {
        let left = panel.x;
        let top = panel.y;
        let right = panel.x + panel.width;
        let bottom = panel.y + panel.height;
        let corner = |path: &mut Builder, vertex: Point, leaving: Point, radius: f32| {
            if radius > 0.0 {
                path.arc_to(vertex, leaving, radius);
            } else {
                path.line_to(vertex);
            }
        };
        Path::new(|path| {
            path.move_to(Point::new(left + corners.top_left, top));
            if self.position == Position::Top {
                self.trace_sides(path, false);
            }
            path.line_to(Point::new(right - corners.top_right, top));
            corner(
                path,
                Point::new(right, top),
                Point::new(right, top + corners.top_right),
                corners.top_right,
            );
            path.line_to(Point::new(right, bottom - corners.bottom_right));
            corner(
                path,
                Point::new(right, bottom),
                Point::new(right - corners.bottom_right, bottom),
                corners.bottom_right,
            );
            if self.position == Position::Bottom {
                self.trace_sides(path, true);
            }
            path.line_to(Point::new(left + corners.bottom_left, bottom));
            corner(
                path,
                Point::new(left, bottom),
                Point::new(left, bottom - corners.bottom_left),
                corners.bottom_left,
            );
            if self.position == Position::Left {
                self.trace_sides(path, true);
            }
            path.line_to(Point::new(left, top + corners.top_left));
            corner(
                path,
                Point::new(left, top),
                Point::new(left + corners.top_left, top),
                corners.top_left,
            );
            path.close();
        })
    }

    pub fn draw_framed_surface(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        surface: FramedSurface,
    ) {
        let FramedSurface {
            frame,
            bounds,
            corners,
            shadow,
        } = surface;
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: corners,
                    ..Border::default()
                },
                shadow,
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
        self.draw_joined(
            renderer,
            bounds,
            corners,
            frame.fill_color(theme),
            frame.outline_border(theme),
        );
    }

    fn draw_joined(
        &self,
        renderer: &mut iced::Renderer,
        panel: Rectangle,
        corners: Radius,
        fill: Color,
        outline: Border,
    ) {
        let reach = outline.width.max(1.0);
        let drawn = panel.union(&self.bounds()).expand(reach);
        let mut frame = Frame::with_bounds(renderer, drawn);
        let surface = self.joined_outline(panel, corners);
        frame.fill(&surface, fill);
        if outline.width > 0.0 {
            frame.stroke(
                &surface,
                Stroke::default()
                    .with_color(outline.color)
                    .with_width(outline.width),
            );
        }
        renderer.draw_geometry(frame.into_geometry());
    }
}
