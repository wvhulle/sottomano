use iced::{
    Border, Color, Point, Rectangle, Size, Vector,
    advanced::graphics::geometry::Renderer as _,
    widget::canvas::{Frame, Path, Stroke},
};

use crate::desktop_shell::monitor_surfaces::Position;

const BASE_HALF_WIDTH_PX: f32 = 6.0;

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
    pub fn between(
        anchor: Rectangle,
        bar_edge: f32,
        popup: Rectangle,
        position: Position,
        corner_radius: f32,
    ) -> Self {
        let inset = BASE_HALF_WIDTH_PX + corner_radius;
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

    pub fn translated(&self, offset: Vector) -> Self {
        Self {
            position: self.position,
            apex: self.apex + offset,
            base_start: self.base_start + offset,
            base_end: self.base_end + offset,
        }
    }

    fn bounds(&self) -> Rectangle {
        let corners = [self.apex, self.base_start, self.base_end];
        let left = corners.iter().map(|corner| corner.x).fold(f32::INFINITY, f32::min);
        let top = corners.iter().map(|corner| corner.y).fold(f32::INFINITY, f32::min);
        let right = corners.iter().map(|corner| corner.x).fold(f32::NEG_INFINITY, f32::max);
        let bottom = corners.iter().map(|corner| corner.y).fold(f32::NEG_INFINITY, f32::max);
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

    pub fn base_span(&self, reach: f32) -> Rectangle {
        match self.position {
            Position::Top => Rectangle {
                x: self.base_start.x,
                y: self.base_start.y,
                width: self.base_end.x - self.base_start.x,
                height: reach,
            },
            Position::Bottom => Rectangle {
                x: self.base_start.x,
                y: self.base_start.y - reach,
                width: self.base_end.x - self.base_start.x,
                height: reach,
            },
            Position::Left => Rectangle {
                x: self.base_start.x,
                y: self.base_start.y,
                width: reach,
                height: self.base_end.y - self.base_start.y,
            },
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
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

    pub fn draw(&self, renderer: &mut iced::Renderer, fill: Color, outline: Border) {
        let mut frame = Frame::with_bounds(renderer, self.bounds());
        let slanted_sides = Path::new(|path| {
            path.move_to(self.base_start);
            path.line_to(self.apex);
            path.line_to(self.base_end);
        });
        frame.fill(&slanted_sides, fill);
        if outline.width > 0.0 {
            frame.stroke(
                &slanted_sides,
                Stroke::default()
                    .with_color(outline.color)
                    .with_width(outline.width),
            );
        }
        renderer.draw_geometry(frame.into_geometry());
    }
}
