use iced::{Rectangle, border::Radius};

use crate::orientation::Edge;

pub(super) fn edge_band(
    edge: Edge,
    slots: Rectangle,
    slot: Rectangle,
    thickness: f32,
) -> Rectangle {
    match edge {
        Edge::Bottom => Rectangle {
            height: thickness,
            ..slot
        },
        Edge::Top => Rectangle {
            y: slot.y + slot.height - thickness,
            height: thickness,
            ..slot
        },
        Edge::Left => Rectangle {
            x: slots.x + slots.width - thickness,
            width: thickness,
            ..slot
        },
        Edge::Right => Rectangle {
            x: slots.x,
            width: thickness,
            ..slot
        },
    }
}

pub(super) fn band_clear_of_corners(edge: Edge, band: Rectangle, corners: Radius) -> Rectangle {
    match edge {
        Edge::Left => Rectangle {
            y: band.y + corners.top_right,
            height: (band.height - corners.top_right - corners.bottom_right).max(0.0),
            ..band
        },
        Edge::Right => Rectangle {
            y: band.y + corners.top_left,
            height: (band.height - corners.top_left - corners.bottom_left).max(0.0),
            ..band
        },
        Edge::Bottom => Rectangle {
            x: band.x + corners.top_left,
            width: (band.width - corners.top_left - corners.top_right).max(0.0),
            ..band
        },
        Edge::Top => Rectangle {
            x: band.x + corners.bottom_left,
            width: (band.width - corners.bottom_left - corners.bottom_right).max(0.0),
            ..band
        },
    }
}

pub(super) fn edge_backdrop(edge: Edge, slots: Rectangle, slot: Rectangle) -> Rectangle {
    match edge {
        Edge::Left | Edge::Right => Rectangle {
            x: slots.x,
            width: slots.width,
            ..slot
        },
        Edge::Top | Edge::Bottom => Rectangle {
            y: slots.y,
            height: slots.height,
            ..slot
        },
    }
}
