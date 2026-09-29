use iced::{Point, Rectangle, widget::canvas::Path};

use crate::wayland_layers::surface::Position;

const BASE_HALF_WIDTH_PX: f32 = 5.0;
pub const OVERLAP_PX: f32 = 2.0;

pub struct Notch {
    pub region: Rectangle,
    pub path: Path,
}

pub fn notch(
    anchor: Rectangle,
    bar_edge: f32,
    popup: Rectangle,
    position: Position,
    corner_radius: f32,
) -> Notch {
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
) -> Notch {
    let popup_below_bar = position == Position::Top;
    let base_y = if popup_below_bar {
        popup.y + OVERLAP_PX
    } else {
        popup.y + popup.height - OVERLAP_PX
    };
    let height = (base_y - bar_edge_y).abs();
    let center = centered_within(anchor.center_x(), popup.x, popup.x + popup.width, inset);

    let region = Rectangle {
        x: center - BASE_HALF_WIDTH_PX,
        y: base_y.min(bar_edge_y),
        width: BASE_HALF_WIDTH_PX * 2.0,
        height,
    };
    let base = if popup_below_bar { height } else { 0.0 };
    let apex_control = if popup_below_bar {
        -height
    } else {
        height * 2.0
    };
    let path = Path::new(|notch| {
        notch.move_to(Point::new(0.0, base));
        notch.quadratic_curve_to(
            Point::new(BASE_HALF_WIDTH_PX, apex_control),
            Point::new(BASE_HALF_WIDTH_PX * 2.0, base),
        );
        notch.close();
    });

    Notch { region, path }
}

fn beside(anchor: Rectangle, bar_edge_x: f32, popup: Rectangle, inset: f32) -> Notch {
    let base_x = popup.x + OVERLAP_PX;
    let width = (base_x - bar_edge_x).abs();
    let center = centered_within(anchor.center_y(), popup.y, popup.y + popup.height, inset);

    let region = Rectangle {
        x: base_x.min(bar_edge_x),
        y: center - BASE_HALF_WIDTH_PX,
        width,
        height: BASE_HALF_WIDTH_PX * 2.0,
    };
    let path = Path::new(|notch| {
        notch.move_to(Point::new(width, 0.0));
        notch.quadratic_curve_to(
            Point::new(-width, BASE_HALF_WIDTH_PX),
            Point::new(width, BASE_HALF_WIDTH_PX * 2.0),
        );
        notch.close();
    });

    Notch { region, path }
}
