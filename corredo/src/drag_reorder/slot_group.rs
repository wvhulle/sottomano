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

    pub(super) fn tile_on_slot(self, pager_axis: Axis, slot: Rectangle) -> renderer::Quad {
        tile_quad(
            self.inner_surface(pager_axis, slot),
            self.inner_radius().into(),
        )
    }
}
