use iced::{Rectangle, border::Radius};

use super::PagerEdge;

pub(super) fn edge_band(
    edge: PagerEdge,
    slots: Rectangle,
    slot: Rectangle,
    thickness: f32,
) -> Rectangle {
    match edge {
        PagerEdge::Bottom => Rectangle {
            height: thickness,
            ..slot
        },
        PagerEdge::Top => Rectangle {
            y: slot.y + slot.height - thickness,
            height: thickness,
            ..slot
        },
        PagerEdge::Left => Rectangle {
            x: slots.x + slots.width - thickness,
            width: thickness,
            ..slot
        },
        PagerEdge::Right => Rectangle {
            x: slots.x,
            width: thickness,
            ..slot
        },
    }
}

pub(super) fn band_clear_of_corners(
    edge: PagerEdge,
    band: Rectangle,
    corners: Radius,
) -> Rectangle {
    match edge {
        PagerEdge::Left => Rectangle {
            y: band.y + corners.top_right,
            height: (band.height - corners.top_right - corners.bottom_right).max(0.0),
            ..band
        },
        PagerEdge::Right => Rectangle {
            y: band.y + corners.top_left,
            height: (band.height - corners.top_left - corners.bottom_left).max(0.0),
            ..band
        },
        PagerEdge::Bottom => Rectangle {
            x: band.x + corners.top_left,
            width: (band.width - corners.top_left - corners.top_right).max(0.0),
            ..band
        },
        PagerEdge::Top => Rectangle {
            x: band.x + corners.bottom_left,
            width: (band.width - corners.bottom_left - corners.bottom_right).max(0.0),
            ..band
        },
    }
}

pub(super) fn edge_backdrop(edge: PagerEdge, slots: Rectangle, slot: Rectangle) -> Rectangle {
    match edge {
        PagerEdge::Left | PagerEdge::Right => Rectangle {
            x: slots.x,
            width: slots.width,
            ..slot
        },
        PagerEdge::Top | PagerEdge::Bottom => Rectangle {
            y: slots.y,
            height: slots.height,
            ..slot
        },
    }
}
