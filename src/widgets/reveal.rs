use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

pub fn reveal_x<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    fraction: f32,
) -> Element<'a, Message> {
    Element::new(RevealX {
        content: content.into(),
        fraction: fraction.clamp(0.0, 1.0),
    })
}

struct RevealX<'a, Message> {
    content: Element<'a, Message>,
    fraction: f32,
}

pub(crate) fn clip_cursor(cursor: Cursor, visible: Rectangle) -> Cursor {
    match cursor.position() {
        Some(position) if (visible.x..=visible.x + visible.width).contains(&position.x) => cursor,
        _ => Cursor::Unavailable,
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for RevealX<'_, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let natural_limits = Limits::new(Size::ZERO, Size::new(f32::INFINITY, limits.max().height));
        let child = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &natural_limits);
        let natural = child.size();
        Node::with_children(
            Size::new(natural.width * self.fraction, natural.height),
            vec![child],
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
        let clip = Rectangle {
            x: bounds.x,
            width: bounds.width,
            y: viewport.y,
            height: viewport.height,
        };
        renderer.with_layer(clip, |renderer| {
            self.content.as_widget().draw(
                tree,
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                cursor,
                viewport,
            );
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            tree,
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
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
            tree,
            event,
            layout.children().next().unwrap(),
            clip_cursor(cursor, layout.bounds()),
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
            tree,
            layout.children().next().unwrap(),
            clip_cursor(cursor, layout.bounds()),
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
        if self.fraction <= 0.0 {
            return None;
        }
        self.content.as_widget_mut().overlay(
            tree,
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
