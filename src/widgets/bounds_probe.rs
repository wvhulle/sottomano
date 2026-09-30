use core::slice;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    window,
};

pub fn bounds_probe<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_rects: impl Fn(Vec<Rectangle>) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(BoundsProbe {
        content: content.into(),
        on_rects: Box::new(on_rects),
        sizing: Sizing::Parent,
    })
}

pub fn natural_bounds_probe<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_rects: impl Fn(Vec<Rectangle>) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(BoundsProbe {
        content: content.into(),
        on_rects: Box::new(on_rects),
        sizing: Sizing::Natural,
    })
}

enum Sizing {
    Parent,
    Natural,
}

impl Sizing {
    fn limits(&self, parent: &Limits) -> Limits {
        match self {
            Sizing::Parent => *parent,
            Sizing::Natural => Limits::new(Size::ZERO, Size::INFINITE),
        }
    }
}

struct BoundsProbe<'a, Message> {
    content: Element<'a, Message>,
    on_rects: Box<dyn Fn(Vec<Rectangle>) -> Message + 'a>,
    sizing: Sizing,
}

#[derive(Default)]
struct Published(Vec<Rectangle>);

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for BoundsProbe<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Published>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Published::default())
    }

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
        let child = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &self.sizing.limits(limits),
        );
        Node::with_children(child.size(), vec![child])
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
        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            let rects: Vec<Rectangle> = layout
                .children()
                .next()
                .unwrap()
                .children()
                .map(|child| child.bounds())
                .collect();
            let published = tree.state.downcast_mut::<Published>();
            if published.0 != rects {
                published.0.clone_from(&rects);
                shell.publish((self.on_rects)(rects));
            }
        }
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
