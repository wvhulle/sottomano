use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    mouse::Cursor,
    widget::canvas::{self, Canvas, Frame, Geometry, Path},
};

use crate::wayland_layers::surface::Position;

const BASE_HALF_WIDTH_PX: f32 = 4.0;
const OVERLAP_PX: f32 = 2.0;

pub fn notch<'a, Message: 'a>(position: Position, fill: Color) -> Element<'a, Message> {
    Canvas::new(Notch { position, fill })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct Notch {
    position: Position,
    fill: Color,
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
        frame.fill(&self.outline(bounds.size()), self.fill);
        vec![frame.into_geometry()]
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
        popup.y + OVERLAP_PX
    } else {
        popup.y + popup.height - OVERLAP_PX
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
    let base_x = popup.x + OVERLAP_PX;
    let center = centered_within(anchor.center_y(), popup.y, popup.y + popup.height, inset);
    Rectangle {
        x: base_x.min(bar_edge_x),
        y: center - BASE_HALF_WIDTH_PX,
        width: (base_x - bar_edge_x).abs(),
        height: BASE_HALF_WIDTH_PX * 2.0,
    }
}
