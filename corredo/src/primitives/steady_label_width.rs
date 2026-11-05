use std::slice;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelShape {
    pub widget_name: &'static str,
    pub character_count: usize,
}

#[must_use]
pub fn steady_label_width<'a, Message: 'a>(
    shape: LabelShape,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    Element::new(SteadyLabelWidth { shape, content })
}

struct WidestLaidOut {
    shape: LabelShape,
    width: f32,
}

struct SteadyLabelWidth<'a, Message> {
    shape: LabelShape,
    content: Element<'a, Message>,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for SteadyLabelWidth<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<WidestLaidOut>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(WidestLaidOut {
            shape: self.shape,
            width: 0.0,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        let widest = tree.state.downcast_mut::<WidestLaidOut>();
        if widest.shape != self.shape {
            *widest = WidestLaidOut {
                shape: self.shape,
                width: 0.0,
            };
        }
        tree.diff_children(slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let child_size = child.size();
        let widest = tree.state.downcast_mut::<WidestLaidOut>();
        widest.width = widest.width.max(child_size.width);
        Node::with_children(Size::new(widest.width, child_size.height), vec![child])
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
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
            layout.children().next().unwrap(),
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
            layout.children().next().unwrap(),
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
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
