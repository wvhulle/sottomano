use core::slice;

use iced::{
    Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer,
        widget::Tree,
    },
    mouse::Cursor,
};

pub fn layout_only<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(LayoutOnly {
        content: content.into(),
    })
}

struct LayoutOnly<'a, Message> {
    content: Element<'a, Message>,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for LayoutOnly<'_, Message> {
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
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            Cursor::Unavailable,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
}
