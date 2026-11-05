use std::ops::Range;

use iced::{
    Color, Element, Rectangle,
    advanced::{Renderer as _, renderer},
    border,
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

    fn tile_quad(self, bounds: Rectangle) -> renderer::Quad {
        renderer::Quad {
            bounds,
            border: border::rounded(self.inner_radius()),
            snap: true,
            ..renderer::Quad::default()
        }
    }

    pub(super) fn tile_on_slot(self, along_vertical_edge: bool, slot: Rectangle) -> renderer::Quad {
        self.tile_quad(self.inner_surface(along_vertical_edge, slot))
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
        renderer.fill_quad(self.tile_inset.tile_quad(self.inner_surface), inner_color);
    }

    pub(super) fn highlight_inside(&self, slot: Rectangle) -> renderer::Quad {
        let bounds = self
            .inner_surface
            .intersection(&slot)
            .unwrap_or(self.inner_surface);
        self.tile_inset.tile_quad(bounds)
    }
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::{SlotGroupFrame, SlotTileInset};

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
        let grouped_tile = frame.highlight_inside(WINDOW_SLOT);
        let ungrouped_tile = TILE_INSET.tile_on_slot(along_vertical_edge, WINDOW_SLOT);
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
}
