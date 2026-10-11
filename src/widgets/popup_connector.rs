use iced::{
    Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    mouse::Cursor,
    widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke},
};

use crate::desktop_shell::monitor_surfaces::Position;

const BASE_HALF_WIDTH_PX: f32 = 4.0;

pub fn notch<'a, Message: 'a>(
    position: Position,
    fill: Color,
    outline: Border,
) -> Element<'a, Message> {
    Canvas::new(Notch {
        position,
        fill,
        outline,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct Notch {
    position: Position,
    fill: Color,
    outline: Border,
}

impl Notch {
    fn outline(&self, size: Size) -> Path {
        let half = BASE_HALF_WIDTH_PX;
        Path::new(|path| match self.position {
            Position::Top => {
                path.move_to(Point::new(0.0, size.height));
                path.line_to(Point::new(half, 0.0));
                path.line_to(Point::new(size.width, size.height));
            }
            Position::Bottom => {
                path.move_to(Point::ORIGIN);
                path.line_to(Point::new(half, size.height));
                path.line_to(Point::new(size.width, 0.0));
            }
            Position::Left => {
                path.move_to(Point::new(size.width, 0.0));
                path.line_to(Point::new(0.0, half));
                path.line_to(Point::new(size.width, size.height));
            }
        })
    }
}

impl<Message> canvas::Program<Message> for Notch {
    type State = ();

    fn draw(
        &self,
        (): &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let slanted_sides = self.outline(bounds.size());
        frame.fill(&slanted_sides, self.fill);
        if self.outline.width > 0.0 {
            frame.stroke(
                &slanted_sides,
                Stroke::default()
                    .with_color(self.outline.color)
                    .with_width(self.outline.width),
            );
        }
        vec![frame.into_geometry()]
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn notch_blur_rows(bounds: Rectangle, position: Position) -> Vec<Rectangle> {
    let center_x = bounds.center_x();
    let center_y = bounds.center_y();
    match position {
        Position::Top | Position::Bottom => {
            let top = bounds.y.ceil() as i32;
            let bottom = (bounds.y + bounds.height).floor() as i32;
            (top..bottom)
                .map(|row| {
                    let from_apex = if position == Position::Top {
                        row as f32 + 1.0 - bounds.y
                    } else {
                        bounds.y + bounds.height - row as f32
                    };
                    let width = bounds.width * (from_apex / bounds.height).min(1.0);
                    Rectangle::new(
                        Point::new(center_x - width / 2.0, row as f32),
                        Size::new(width, 1.0),
                    )
                })
                .collect()
        }
        Position::Left => {
            let left = bounds.x.ceil() as i32;
            let right = (bounds.x + bounds.width).floor() as i32;
            (left..right)
                .map(|column| {
                    let from_apex = column as f32 + 1.0 - bounds.x;
                    let height = bounds.height * (from_apex / bounds.width).min(1.0);
                    Rectangle::new(
                        Point::new(column as f32, center_y - height / 2.0),
                        Size::new(1.0, height),
                    )
                })
                .collect()
        }
    }
}

pub fn notch_region(
    anchor: Rectangle,
    bar_edge: f32,
    popup: Rectangle,
    position: Position,
    corner_radius: f32,
) -> Rectangle {
    let inset = BASE_HALF_WIDTH_PX + corner_radius;
    match position {
        Position::Top | Position::Bottom => {
            below_or_above(anchor, bar_edge, popup, position, inset)
        }
        Position::Left => beside(anchor, bar_edge, popup, inset),
    }
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

fn below_or_above(
    anchor: Rectangle,
    bar_edge_y: f32,
    popup: Rectangle,
    position: Position,
    inset: f32,
) -> Rectangle {
    let base_y = if position == Position::Top {
        popup.y
    } else {
        popup.y + popup.height
    };
    let center = centered_within(anchor.center_x(), popup.x, popup.x + popup.width, inset);
    Rectangle {
        x: center - BASE_HALF_WIDTH_PX,
        y: base_y.min(bar_edge_y),
        width: BASE_HALF_WIDTH_PX * 2.0,
        height: (base_y - bar_edge_y).abs(),
    }
}

fn beside(anchor: Rectangle, bar_edge_x: f32, popup: Rectangle, inset: f32) -> Rectangle {
    let base_x = popup.x;
    let center = centered_within(anchor.center_y(), popup.y, popup.y + popup.height, inset);
    Rectangle {
        x: base_x.min(bar_edge_x),
        y: center - BASE_HALF_WIDTH_PX,
        width: (base_x - bar_edge_x).abs(),
        height: BASE_HALF_WIDTH_PX * 2.0,
    }
}
