use core::slice;

use iced::{
    Border, Color, Element, Event, Length, Padding, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    border::Radius,
    mouse::Cursor,
    widget::container,
};

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct OutlinedSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl OutlinedSides {
    pub const ALL: Self = Self {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };

    fn square_unoutlined_corners(self, radius: Radius) -> Radius {
        let rounded_between = |first: bool, second: bool, corner: f32| {
            if first && second { corner } else { 0.0 }
        };
        Radius {
            top_left: rounded_between(self.top, self.left, radius.top_left),
            top_right: rounded_between(self.top, self.right, radius.top_right),
            bottom_right: rounded_between(self.bottom, self.right, radius.bottom_right),
            bottom_left: rounded_between(self.bottom, self.left, radius.bottom_left),
        }
    }

    fn push_unoutlined_past(self, bounds: Rectangle, outline_width: f32) -> Rectangle {
        let beyond_antialiasing = |outlined: bool| if outlined { 0.0 } else { 2.0 * outline_width };
        bounds.expand(Padding {
            top: beyond_antialiasing(self.top),
            right: beyond_antialiasing(self.right),
            bottom: beyond_antialiasing(self.bottom),
            left: beyond_antialiasing(self.left),
        })
    }
}

pub fn panel_frame<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    style: &container::Style,
) -> PanelFrame<'a, Message> {
    PanelFrame {
        content: content.into(),
        style: *style,
        outlined_sides: OutlinedSides::ALL,
    }
}

pub struct PanelFrame<'a, Message> {
    content: Element<'a, Message>,
    style: container::Style,
    outlined_sides: OutlinedSides,
}

impl<Message> PanelFrame<'_, Message> {
    #[must_use]
    pub fn outlined_sides(mut self, sides: OutlinedSides) -> Self {
        self.outlined_sides = sides;
        self
    }

    fn draw_fill(&self, renderer: &mut iced::Renderer, bounds: Rectangle, radius: Radius) {
        let fill_without_outline = container::Style {
            border: Border {
                width: 0.0,
                radius,
                ..self.style.border
            },
            ..self.style
        };
        container::draw_background(renderer, &fill_without_outline, bounds);
    }

    fn draw_outline(&self, renderer: &mut iced::Renderer, bounds: Rectangle, radius: Radius) {
        let outline = self.style.border;
        if outline.width <= 0.0 {
            return;
        }
        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: self
                        .outlined_sides
                        .push_unoutlined_past(bounds, outline.width),
                    border: Border { radius, ..outline },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        });
    }
}

impl<'a, Message: 'a> From<PanelFrame<'a, Message>> for Element<'a, Message> {
    fn from(frame: PanelFrame<'a, Message>) -> Self {
        Self::new(frame)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for PanelFrame<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
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
        let radius = self
            .outlined_sides
            .square_unoutlined_corners(self.style.border.radius);
        self.draw_fill(renderer, bounds, radius);
        let framed_style = renderer::Style {
            text_color: self.style.text_color.unwrap_or(style.text_color),
        };
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &framed_style,
            layout,
            cursor,
            viewport,
        );
        self.draw_outline(renderer, bounds, radius);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
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
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
