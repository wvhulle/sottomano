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
    bar_edge_y: f32,
    popup: Rectangle,
    position: Position,
    corner_radius: f32,
) -> Notch {
    let base_y = match position {
        Position::Top => popup.y + OVERLAP_PX,
        Position::Bottom => popup.y + popup.height - OVERLAP_PX,
    };
    let height = (base_y - bar_edge_y).abs();

    let inset = BASE_HALF_WIDTH_PX + corner_radius;
    let leftmost = popup.x + inset;
    let rightmost = popup.x + popup.width - inset;
    let center = if leftmost <= rightmost {
        anchor.center_x().clamp(leftmost, rightmost)
    } else {
        popup.center_x()
    };

    let region = Rectangle {
        x: center - BASE_HALF_WIDTH_PX,
        y: base_y.min(bar_edge_y),
        width: BASE_HALF_WIDTH_PX * 2.0,
        height,
    };
    let (base, apex_control) = match position {
        Position::Top => (height, -height),
        Position::Bottom => (0.0, height * 2.0),
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
