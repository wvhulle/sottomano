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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PagerAxis {
    Horizontal,
    Vertical,
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
    fn inner_surface(self, pager_axis: PagerAxis, slots: Rectangle) -> Rectangle {
        if pager_axis == PagerAxis::Vertical {
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

    fn joined_tile_corners(self, pager_axis: PagerAxis, tile_neighbours: TileNeighbours) -> Radius {
        let rounded = Radius::from(self.inner_radius());
        let leading_joined = match (tile_neighbours.leading, pager_axis) {
            (false, _) => rounded,
            (true, PagerAxis::Horizontal) => rounded.left(0.0),
            (true, PagerAxis::Vertical) => rounded.top(0.0),
        };
        match (tile_neighbours.trailing, pager_axis) {
            (false, _) => leading_joined,
            (true, PagerAxis::Horizontal) => leading_joined.right(0.0),
            (true, PagerAxis::Vertical) => leading_joined.bottom(0.0),
        }
    }

    pub(super) fn tile_on_slot(
        self,
        pager_axis: PagerAxis,
        slot: Rectangle,
        tile_neighbours: TileNeighbours,
    ) -> renderer::Quad {
        tile_quad(
            self.inner_surface(pager_axis, slot),
            self.joined_tile_corners(pager_axis, tile_neighbours),
        )
    }
}

pub(super) struct SlotGroupFrame {
    pager_axis: PagerAxis,
    heading: Rectangle,
    slot_cells: Vec<Rectangle>,
    tile_inset: SlotTileInset,
}

impl SlotGroupFrame {
    pub(super) fn around_slots(
        pager_axis: PagerAxis,
        heading: Rectangle,
        slots: impl IntoIterator<Item = Rectangle>,
        tile_inset: SlotTileInset,
    ) -> Self {
        Self {
            pager_axis,
            heading,
            slot_cells: slots
                .into_iter()
                .map(|slot| tile_inset.inner_surface(pager_axis, slot))
                .collect(),
            tile_inset,
        }
    }

    fn enclosure(&self) -> Option<Rectangle> {
        let around_slot_cells = self
            .slot_cells
            .iter()
            .copied()
            .reduce(|covered, slot_cell| covered.union(&slot_cell))?
            .expand(self.tile_inset.thickness);
        let heading_along_panel = if self.pager_axis == PagerAxis::Vertical {
            Rectangle {
                y: self.heading.y,
                height: self.heading.height,
                ..around_slot_cells
            }
        } else {
            Rectangle {
                x: self.heading.x,
                width: self.heading.width,
                ..around_slot_cells
            }
        };
        Some(around_slot_cells.union(&heading_along_panel))
    }

    pub(super) fn draw(
        &self,
        renderer: &mut iced::Renderer,
        enclosure_color: Color,
        inner_color: Color,
    ) {
        let Some(enclosure) = self.enclosure() else {
            return;
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: enclosure,
                border: border::rounded(self.tile_inset.enclosure_radius),
                snap: true,
                ..renderer::Quad::default()
            },
            enclosure_color,
        );
        for slot_cell in &self.slot_cells {
            renderer.fill_quad(
                tile_quad(*slot_cell, self.tile_inset.inner_radius().into()),
                inner_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::{Rectangle, border::Radius};

    use super::{PagerAxis, SlotGroupFrame, SlotTileInset, TileNeighbours};

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

    #[test]
    fn each_grouped_slot_gets_its_own_cell_inside_the_enclosure() {
        let next_slot = Rectangle {
            x: WINDOW_SLOT.x + WINDOW_SLOT.width + TILE_INSET.thickness,
            ..WINDOW_SLOT
        };
        let frame = SlotGroupFrame::around_slots(
            PagerAxis::Horizontal,
            Rectangle::default(),
            [WINDOW_SLOT, next_slot],
            TILE_INSET,
        );
        assert_eq!(
            frame.slot_cells,
            [
                TILE_INSET.inner_surface(PagerAxis::Horizontal, WINDOW_SLOT),
                TILE_INSET.inner_surface(PagerAxis::Horizontal, next_slot),
            ]
        );
        let enclosure = frame.enclosure().expect("two slots enclose a group");
        assert!(
            frame
                .slot_cells
                .iter()
                .all(|slot_cell| enclosure.contains(slot_cell.center()))
        );
    }

    #[test]
    fn tile_without_neighbours_keeps_every_inner_corner() {
        let corners =
            TILE_INSET.joined_tile_corners(PagerAxis::Horizontal, TileNeighbours::default());
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
            TILE_INSET.joined_tile_corners(PagerAxis::Horizontal, leading_only),
            Radius::default().right(5.0)
        );
        assert_eq!(
            TILE_INSET.joined_tile_corners(PagerAxis::Horizontal, trailing_only),
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
            TILE_INSET.joined_tile_corners(PagerAxis::Vertical, leading_only),
            Radius::default().bottom(5.0)
        );
        assert_eq!(
            TILE_INSET.joined_tile_corners(PagerAxis::Vertical, trailing_only),
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
            TILE_INSET.joined_tile_corners(PagerAxis::Horizontal, both_sides),
            Radius::default()
        );
    }
}
