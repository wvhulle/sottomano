use std::ops::Range;

use iced::{
    Color, Element, Rectangle,
    advanced::{Renderer as _, renderer},
    border,
};

use crate::widgets::configured_theme::ThemedColor;

pub struct SlotGroup<'a, Message> {
    pub positions: Range<usize>,
    pub heading: Element<'a, Message>,
    pub enclosure_color: ThemedColor<'a>,
    pub inner_color: ThemedColor<'a>,
    pub highlight_color: ThemedColor<'a>,
    pub enclosure_thickness: f32,
    pub enclosure_radius: f32,
}

pub(super) struct SlotGroupFrame {
    along_vertical_edge: bool,
    heading: Rectangle,
    inner_surface: Rectangle,
    thickness: f32,
    enclosure_radius: f32,
}

impl SlotGroupFrame {
    pub(super) fn around_slots(
        along_vertical_edge: bool,
        heading: Rectangle,
        slots: Rectangle,
        thickness: f32,
        enclosure_radius: f32,
    ) -> Self {
        let inner_surface = if along_vertical_edge {
            Rectangle {
                x: slots.x + thickness,
                width: (slots.width - 2.0 * thickness).max(0.0),
                ..slots
            }
        } else {
            Rectangle {
                y: slots.y + thickness,
                height: (slots.height - 2.0 * thickness).max(0.0),
                ..slots
            }
        };
        Self {
            along_vertical_edge,
            heading,
            inner_surface,
            thickness,
            enclosure_radius,
        }
    }

    fn enclosure(&self) -> Rectangle {
        let around_inner_surface = self.inner_surface.expand(self.thickness);
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

    fn inner_radius(&self) -> f32 {
        (self.enclosure_radius - self.thickness).max(0.0)
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
                border: border::rounded(self.enclosure_radius),
                snap: true,
                ..renderer::Quad::default()
            },
            enclosure_color,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: self.inner_surface,
                border: border::rounded(self.inner_radius()),
                snap: true,
                ..renderer::Quad::default()
            },
            inner_color,
        );
    }

    pub(super) fn highlight_inside(&self, slot: Rectangle) -> renderer::Quad {
        let bounds = self
            .inner_surface
            .intersection(&slot)
            .unwrap_or(self.inner_surface);
        renderer::Quad {
            bounds,
            border: border::rounded(self.inner_radius()),
            snap: true,
            ..renderer::Quad::default()
        }
    }
}
