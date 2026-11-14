use std::ops::Range;

use iced::{
    Element, Rectangle,
    advanced::renderer,
    border::{Border, Radius},
};

use crate::{orientation::Axis, theming::ThemedColor};

pub struct SlotGroup<'a, Message> {
    pub positions: Range<usize>,
    pub heading: Element<'a, Message>,
    pub enclosure_color: ThemedColor<'a>,
    pub inner_color: ThemedColor<'a>,
    pub highlight_color: ThemedColor<'a>,
    pub enclosure_thickness: f32,
    pub enclosure_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotTileInset {
    pub thickness: f32,
    pub enclosure_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct TileNeighbours {
    pub(super) leading: bool,
    pub(super) trailing: bool,
}

pub(super) fn tile_quad(bounds: Rectangle, corners: Radius) -> renderer::Quad {
    renderer::Quad {
        bounds,
        border: Border {
            radius: corners,
            ..Border::default()
        },
        snap: true,
        ..renderer::Quad::default()
    }
}

impl SlotTileInset {
    pub(super) fn inner_surface(self, pager_axis: Axis, slots: Rectangle) -> Rectangle {
        if pager_axis == Axis::Vertical {
            Rectangle {
                x: slots.x + self.thickness,
                width: (slots.width - 2.0 * self.thickness).max(0.0),
                ..slots
            }
        } else {
            Rectangle {
                y: slots.y + self.thickness,
                height: (slots.height - 2.0 * self.thickness).max(0.0),
                ..slots
            }
        }
    }

    pub(super) fn inner_radius(self) -> f32 {
        (self.enclosure_radius - self.thickness).max(0.0)
    }

    fn joined_tile_corners(self, pager_axis: Axis, tile_neighbours: TileNeighbours) -> Radius {
        let rounded = Radius::from(self.inner_radius());
        let leading_joined = match (tile_neighbours.leading, pager_axis) {
            (false, _) => rounded,
            (true, Axis::Horizontal) => rounded.left(0.0),
            (true, Axis::Vertical) => rounded.top(0.0),
        };
        match (tile_neighbours.trailing, pager_axis) {
            (false, _) => leading_joined,
            (true, Axis::Horizontal) => leading_joined.right(0.0),
            (true, Axis::Vertical) => leading_joined.bottom(0.0),
        }
    }

    pub(super) fn tile_on_slot(
        self,
        pager_axis: Axis,
        slot: Rectangle,
        tile_neighbours: TileNeighbours,
    ) -> renderer::Quad {
        tile_quad(
            self.inner_surface(pager_axis, slot),
            self.joined_tile_corners(pager_axis, tile_neighbours),
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::border::Radius;

    use super::{SlotTileInset, TileNeighbours};
    use crate::orientation::Axis;

    const TILE_INSET: SlotTileInset = SlotTileInset {
        thickness: 3.0,
        enclosure_radius: 8.0,
    };

    #[test]
    fn tile_without_neighbours_keeps_every_inner_corner() {
        let corners = TILE_INSET.joined_tile_corners(Axis::Horizontal, TileNeighbours::default());
        assert_eq!(corners, Radius::from(5.0));
    }

    #[test]
    fn horizontal_tile_squares_the_side_facing_each_neighbour() {
        let leading_only = TileNeighbours {
            leading: true,
            trailing: false,
        };
        let trailing_only = TileNeighbours {
            leading: false,
            trailing: true,
        };
        assert_eq!(
            TILE_INSET.joined_tile_corners(Axis::Horizontal, leading_only),
            Radius::default().right(5.0)
        );
        assert_eq!(
            TILE_INSET.joined_tile_corners(Axis::Horizontal, trailing_only),
            Radius::default().left(5.0)
        );
    }

    #[test]
    fn vertical_tile_squares_the_side_facing_each_neighbour() {
        let leading_only = TileNeighbours {
            leading: true,
            trailing: false,
        };
        let trailing_only = TileNeighbours {
            leading: false,
            trailing: true,
        };
        assert_eq!(
            TILE_INSET.joined_tile_corners(Axis::Vertical, leading_only),
            Radius::default().bottom(5.0)
        );
        assert_eq!(
            TILE_INSET.joined_tile_corners(Axis::Vertical, trailing_only),
            Radius::default().top(5.0)
        );
    }

    #[test]
    fn tile_between_two_neighbours_has_no_rounded_corners() {
        let both_sides = TileNeighbours {
            leading: true,
            trailing: true,
        };
        assert_eq!(
            TILE_INSET.joined_tile_corners(Axis::Horizontal, both_sides),
            Radius::default()
        );
    }
}
