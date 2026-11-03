use std::{f32::consts::PI, slice::from_ref};

use iced::{
    Border, Color, Element, Event, Length, Point, Radians, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    border::Radius,
    gradient::Linear,
    mouse::Cursor,
};

use crate::{
    desktop_panels::panel_frame::LayerFrameStyle,
    widgets::{reveal::clip_cursor, shared_metrics::SCROLL_LINE_STEP_PX},
};

#[must_use]
fn clamped_scroll(scrolled_px: f32, content_height: f32, visible_height: f32) -> f32 {
    scrolled_px.clamp(0.0, (content_height - visible_height).max(0.0))
}

pub fn menu_rows_viewport<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    scrolled_px: f32,
    on_scroll: impl Fn(f32) -> Message + 'a,
    edge_fade: RowsEdgeFade,
) -> Element<'a, Message> {
    Element::new(MenuRowsViewport {
        content: content.into(),
        scrolled_px,
        on_scroll: Box::new(on_scroll),
        edge_fade,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct RowsEdgeFade {
    pub frame: LayerFrameStyle,
    pub height_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HiddenRowEdges {
    above: bool,
    below: bool,
}

impl HiddenRowEdges {
    fn of(scrolled_px: f32, content_height: f32, visible_height: f32) -> Self {
        let scrolled = clamped_scroll(scrolled_px, content_height, visible_height);
        Self {
            above: scrolled > 0.0,
            below: scrolled + visible_height < content_height,
        }
    }
}

impl RowsEdgeFade {
    fn draw_at_top(self, renderer: &mut iced::Renderer, theme: &iced::Theme, bounds: Rectangle) {
        let corners = self.frame.corner_radius();
        self.draw_band(
            renderer,
            theme,
            Rectangle {
                height: self.height_px.min(bounds.height),
                ..bounds
            },
            Radians(0.0),
            Radius {
                bottom_left: 0.0,
                bottom_right: 0.0,
                ..corners
            },
        );
    }

    fn draw_at_bottom(self, renderer: &mut iced::Renderer, theme: &iced::Theme, bounds: Rectangle) {
        let band_height = self.height_px.min(bounds.height);
        let corners = self.frame.corner_radius();
        self.draw_band(
            renderer,
            theme,
            Rectangle {
                y: bounds.y + bounds.height - band_height,
                height: band_height,
                ..bounds
            },
            Radians(PI),
            Radius {
                top_left: 0.0,
                top_right: 0.0,
                ..corners
            },
        );
    }

    fn draw_band(
        self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        band: Rectangle,
        solid_end_direction: Radians,
        corners: Radius,
    ) {
        let solid = self.frame.fill_color(theme);
        renderer.fill_quad(
            renderer::Quad {
                bounds: band,
                border: Border {
                    radius: corners,
                    ..Border::default()
                },
                ..renderer::Quad::default()
            },
            Linear::new(solid_end_direction)
                .add_stop(0.0, Color { a: 0.0, ..solid })
                .add_stop(1.0, solid),
        );
    }
}

struct MenuRowsViewport<'a, Message> {
    content: Element<'a, Message>,
    scrolled_px: f32,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
    edge_fade: RowsEdgeFade,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for MenuRowsViewport<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let visible_budget = limits.max().height;
        let natural_limits = Limits::new(Size::ZERO, Size::new(limits.max().width, f32::INFINITY));
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &natural_limits);
        let natural = child.size();
        let visible_height = natural.height.min(visible_budget);
        let scrolled = clamped_scroll(self.scrolled_px, natural.height, visible_height);
        Node::with_children(
            Size::new(natural.width, visible_height),
            vec![child.move_to(Point::new(0.0, -scrolled))],
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(content_clip) = bounds.intersection(viewport) else {
            return;
        };
        renderer.with_layer(content_clip, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                clip_cursor(cursor, bounds),
                &content_clip,
            );
        });
        let content_height = layout.children().next().unwrap().bounds().height;
        let hidden_edges = HiddenRowEdges::of(self.scrolled_px, content_height, bounds.height);
        renderer.with_layer(content_clip, |renderer| {
            if hidden_edges.above {
                self.edge_fade.draw_at_top(renderer, theme, bounds);
            }
            if hidden_edges.below {
                self.edge_fade.draw_at_bottom(renderer, theme, bounds);
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let child_layout = layout.children().next().unwrap();
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            child_layout,
            clip_cursor(cursor, bounds),
            renderer,
            clipboard,
            shell,
            &bounds,
        );
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && !shell.is_event_captured()
            && cursor.position_over(bounds).is_some()
        {
            let travel = match *delta {
                mouse::ScrollDelta::Lines { y, .. } => y * SCROLL_LINE_STEP_PX,
                mouse::ScrollDelta::Pixels { y, .. } => y,
            };
            let content_height = child_layout.bounds().height;
            let current = clamped_scroll(self.scrolled_px, content_height, bounds.height);
            let next = clamped_scroll(current - travel, content_height, bounds.height);
            if (next - current).abs() > f32::EPSILON {
                shell.publish((self.on_scroll)(next));
                shell.capture_event();
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            clip_cursor(cursor, bounds),
            &bounds,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{HiddenRowEdges, clamped_scroll};

    #[test]
    fn only_edges_with_hidden_rows_are_faded() {
        let hidden = |scrolled_px, content_height, visible_height| {
            let edges = HiddenRowEdges::of(scrolled_px, content_height, visible_height);
            [edges.above, edges.below]
        };
        assert_eq!(hidden(0.0, 80.0, 100.0), [false, false]);
        assert_eq!(hidden(0.0, 300.0, 100.0), [false, true]);
        assert_eq!(hidden(50.0, 300.0, 100.0), [true, true]);
        assert_eq!(hidden(500.0, 300.0, 100.0), [true, false]);
    }

    #[test]
    fn scroll_is_clamped_to_the_overflow() {
        assert!((clamped_scroll(500.0, 300.0, 100.0) - 200.0).abs() < f32::EPSILON);
        assert!(clamped_scroll(-20.0, 300.0, 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn content_that_fits_never_scrolls() {
        assert!(clamped_scroll(50.0, 80.0, 100.0).abs() < f32::EPSILON);
    }
}
