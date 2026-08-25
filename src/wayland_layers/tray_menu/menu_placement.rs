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

use crate::{app::Message, wayland_layers::surface::Position};

pub fn anchored_menu<'a>(
    content: impl Into<Element<'a, Message>>,
    anchor_left: f32,
    position: Position,
    edge_inset: f32,
    on_bounds: impl Fn(Rectangle) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(AnchoredMenu {
        content: content.into(),
        anchor_left,
        position,
        edge_inset,
        on_bounds: Box::new(on_bounds),
    })
}

struct AnchoredMenu<'a> {
    content: Element<'a, Message>,
    anchor_left: f32,
    position: Position,
    edge_inset: f32,
    on_bounds: Box<dyn Fn(Rectangle) -> Message + 'a>,
}

#[derive(Default)]
struct Published(Option<Rectangle>);

impl AnchoredMenu<'_> {
    fn origin(&self, size: Size, surface: Size) -> Vector {
        let desired_y = match self.position {
            Position::Top => self.edge_inset,
            Position::Bottom => surface.height - size.height - self.edge_inset,
        };
        Vector::new(
            self.anchor_left
                .clamp(0.0, (surface.width - size.width).max(0.0)),
            desired_y.clamp(0.0, (surface.height - size.height).max(0.0)),
        )
    }
}

impl Widget<Message, iced::Theme, iced::Renderer> for AnchoredMenu<'_> {
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
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let surface = limits.max();
        let child = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &Limits::new(Size::ZERO, surface),
        );
        let origin = self.origin(child.size(), surface);
        Node::with_children(surface, vec![child.translate(origin)])
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
        if !matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            return;
        }
        let bounds = layout.children().next().unwrap().bounds();
        let published = tree.state.downcast_mut::<Published>();
        if published.0 != Some(bounds) {
            published.0 = Some(bounds);
            shell.publish((self.on_bounds)(bounds));
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
