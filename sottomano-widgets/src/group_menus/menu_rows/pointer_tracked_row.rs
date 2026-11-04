use core::slice;

use iced::{
    Border, Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::{Cursor, Interaction},
};

use crate::{
    group_menus::menu_rows::menu_row::{MenuRowLook, menu_row_background},
    widgets::configured_theme::text_color,
};

pub(crate) struct PointerTrackedRow<'a, Message> {
    body: Element<'a, Message>,
    look: MenuRowLook,
}

impl<'a, Message: 'a> PointerTrackedRow<'a, Message> {
    pub(crate) fn new(body: impl Into<Element<'a, Message>>, look: MenuRowLook) -> Self {
        Self {
            body: body.into(),
            look,
        }
    }
}

impl<'a, Message: 'a> From<PointerTrackedRow<'a, Message>> for Element<'a, Message> {
    fn from(row: PointerTrackedRow<'a, Message>) -> Self {
        Element::new(row)
    }
}

#[derive(Default)]
struct RowPointerState {
    pointer_over: bool,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for PointerTrackedRow<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<RowPointerState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(RowPointerState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.body)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.body));
    }

    fn size(&self) -> Size<Length> {
        self.body.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let body_node = self
            .body
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(body_node.size(), vec![body_node])
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
        self.body.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let pointer_over = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<RowPointerState>();
        if state.pointer_over != pointer_over {
            state.pointer_over = pointer_over;
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<RowPointerState>();
        if let Some(background) = menu_row_background(theme, self.look.emphasis, state.pointer_over)
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        radius: self.look.corners,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                background,
            );
        }
        self.body.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style {
                text_color: text_color(theme),
            },
            layout.children().next().unwrap(),
            cursor,
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
    ) -> Interaction {
        self.body.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
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
        self.body.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'overlay>(
        &'overlay mut self,
        tree: &'overlay mut Tree,
        layout: Layout<'overlay>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'overlay, Message, Theme, iced::Renderer>> {
        self.body.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
