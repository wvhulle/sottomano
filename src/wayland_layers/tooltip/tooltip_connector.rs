use iced::{Point, Rectangle, widget::canvas::Path};

use crate::wayland_layers::surface::Position;

const BASE_HALF_WIDTH_PX: f32 = 5.0;
pub const TOOLTIP_OVERLAP_PX: f32 = 2.0;

pub struct Notch {
    pub region: Rectangle,
    pub path: Path,
}

pub fn notch(
    anchor: Rectangle,
    bar_edge_y: f32,
    tooltip: Rectangle,
    position: Position,
    corner_radius: f32,
) -> Notch {
    let base_y = match position {
        Position::Top => tooltip.y + TOOLTIP_OVERLAP_PX,
        Position::Bottom => tooltip.y + tooltip.height - TOOLTIP_OVERLAP_PX,
    };
    let height = (base_y - bar_edge_y).abs();

    let inset = BASE_HALF_WIDTH_PX + corner_radius;
    let leftmost = tooltip.x + inset;
    let rightmost = tooltip.x + tooltip.width - inset;
    let center = if leftmost <= rightmost {
        anchor.center_x().clamp(leftmost, rightmost)
    } else {
        tooltip.center_x()
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
