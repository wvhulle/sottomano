use iced::{
    Event, Size, Theme,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::Cursor,
};

use crate::popovers::AnchoredPopup;

pub(super) struct PopupOverlay<'tree, 'view, Message> {
    pub(super) popup: &'tree mut AnchoredPopup<'view, Message>,
    pub(super) tree: &'tree mut Tree,
}

impl<Message: Clone> overlay::Overlay<Message, Theme, iced::Renderer>
    for PopupOverlay<'_, '_, Message>
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        Widget::<Message, Theme, iced::Renderer>::layout(
            self.popup,
            self.tree,
            renderer,
            &Limits::new(Size::ZERO, bounds),
        )
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        Widget::<Message, Theme, iced::Renderer>::draw(
            &*self.popup,
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        Widget::<Message, Theme, iced::Renderer>::operate(
            self.popup, self.tree, layout, renderer, operation,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let viewport = layout.bounds();
        Widget::<Message, Theme, iced::Renderer>::update(
            self.popup, self.tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        Widget::<Message, Theme, iced::Renderer>::mouse_interaction(
            &*self.popup,
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }
}
