use std::ops::Range;

use iced::{
    Color, Element, Rectangle,
    advanced::{Renderer as _, renderer},
    border,
    border::{Border, Radius},
};

use crate::theme_colors::ThemedColor;

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

fn tile_quad(bounds: Rectangle, corners: Radius) -> renderer::Quad {
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
    fn inner_surface(self, along_vertical_edge: bool, slots: Rectangle) -> Rectangle {
        if along_vertical_edge {
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

    fn inner_radius(self) -> f32 {
        (self.enclosure_radius - self.thickness).max(0.0)
    }

    fn joined_tile_corners(
        self,
        along_vertical_edge: bool,
        tile_neighbours: TileNeighbours,
    ) -> Radius {
        let rounded = Radius::from(self.inner_radius());
        let leading_joined = match (tile_neighbours.leading, along_vertical_edge) {
            (false, _) => rounded,
            (true, false) => rounded.left(0.0),
            (true, true) => rounded.top(0.0),
        };
        match (tile_neighbours.trailing, along_vertical_edge) {
            (false, _) => leading_joined,
            (true, false) => leading_joined.right(0.0),
            (true, true) => leading_joined.bottom(0.0),
        }
    }

    pub(super) fn tile_on_slot(
        self,
        along_vertical_edge: bool,
        slot: Rectangle,
        tile_neighbours: TileNeighbours,
    ) -> renderer::Quad {
        tile_quad(
            self.inner_surface(along_vertical_edge, slot),
            self.joined_tile_corners(along_vertical_edge, tile_neighbours),
        )
    }
}

pub(super) struct SlotGroupFrame {
    along_vertical_edge: bool,
    heading: Rectangle,
    inner_surface: Rectangle,
    tile_inset: SlotTileInset,
}

impl SlotGroupFrame {
    pub(super) fn around_slots(
        along_vertical_edge: bool,
        heading: Rectangle,
        slots: Rectangle,
        tile_inset: SlotTileInset,
    ) -> Self {
        Self {
            along_vertical_edge,
            heading,
            inner_surface: tile_inset.inner_surface(along_vertical_edge, slots),
            tile_inset,
        }
    }

    fn enclosure(&self) -> Rectangle {
        let around_inner_surface = self.inner_surface.expand(self.tile_inset.thickness);
        let heading_along_panel = if self.along_vertical_edge {
            Rectangle {
                y: self.heading.y,
                height: self.heading.height,
                ..around_inner_surface
            }
        } else {
            Rectangle {
                x: self.heading.x,
                width: self.heading.width,
                ..around_inner_surface
            }
        };
        around_inner_surface.union(&heading_along_panel)
    }

    pub(super) fn draw(
        &self,
        renderer: &mut iced::Renderer,
        enclosure_color: Color,
        inner_color: Color,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: self.enclosure(),
                border: border::rounded(self.tile_inset.enclosure_radius),
                snap: true,
                ..renderer::Quad::default()
            },
            enclosure_color,
        );
        renderer.fill_quad(
            tile_quad(self.inner_surface, self.tile_inset.inner_radius().into()),
            inner_color,
        );
    }

    pub(super) fn highlight_inside(
        &self,
        slot: Rectangle,
        tile_neighbours: TileNeighbours,
    ) -> renderer::Quad {
        let bounds = self
            .inner_surface
            .intersection(&slot)
            .unwrap_or(self.inner_surface);
        tile_quad(
            bounds,
            self.tile_inset
                .joined_tile_corners(self.along_vertical_edge, tile_neighbours),
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::{Rectangle, border::Radius};

    use super::{SlotGroupFrame, SlotTileInset, TileNeighbours};

    const TILE_INSET: SlotTileInset = SlotTileInset {
        thickness: 3.0,
        enclosure_radius: 8.0,
    };
    const WINDOW_SLOT: Rectangle = Rectangle {
        x: 40.0,
        y: 0.0,
        width: 120.0,
        height: 30.0,
    };

    fn assert_tiles_match_across_grouping(along_vertical_edge: bool) {
        let frame = SlotGroupFrame::around_slots(
            along_vertical_edge,
            Rectangle::default(),
            WINDOW_SLOT,
            TILE_INSET,
        );
        let grouped_tile = frame.highlight_inside(WINDOW_SLOT, TileNeighbours::default());
        let ungrouped_tile =
            TILE_INSET.tile_on_slot(along_vertical_edge, WINDOW_SLOT, TileNeighbours::default());
        assert_eq!(ungrouped_tile.bounds, grouped_tile.bounds);
        assert_eq!(ungrouped_tile.border, grouped_tile.border);
        assert_eq!(ungrouped_tile.snap, grouped_tile.snap);
    }

    #[test]
    fn ungrouped_tile_matches_grouped_tile_on_horizontal_panels() {
        assert_tiles_match_across_grouping(false);
    }

    #[test]
    fn ungrouped_tile_matches_grouped_tile_on_vertical_panels() {
        assert_tiles_match_across_grouping(true);
    }

    #[test]
    fn tile_without_neighbours_keeps_every_inner_corner() {
        let corners = TILE_INSET.joined_tile_corners(false, TileNeighbours::default());
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
            TILE_INSET.joined_tile_corners(false, leading_only),
            Radius::default().right(5.0)
        );
        assert_eq!(
            TILE_INSET.joined_tile_corners(false, trailing_only),
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
            TILE_INSET.joined_tile_corners(true, leading_only),
            Radius::default().bottom(5.0)
        );
        assert_eq!(
            TILE_INSET.joined_tile_corners(true, trailing_only),
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
            TILE_INSET.joined_tile_corners(false, both_sides),
            Radius::default()
        );
    }
}
