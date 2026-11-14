use std::any::Any;

use iced::{
    Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{
            Id, Operation, Tree,
            operation::{Focusable, Scrollable, TextInput},
            tree,
        },
    },
    mouse::Cursor,
};

type PayloadTranslation<'a, Payload> = Box<dyn Fn(&Payload, &mut dyn Operation) + Send + Sync + 'a>;

pub(crate) struct RegionTranslation<'a, Message, Payload> {
    content: Element<'a, Message>,
    translate: PayloadTranslation<'a, Payload>,
}

pub(crate) fn translated_regions<'a, Message: 'a, Payload: 'static>(
    content: impl Into<Element<'a, Message>>,
    translate: impl Fn(&Payload, &mut dyn Operation) + Send + Sync + 'a,
) -> Element<'a, Message> {
    Element::new(RegionTranslation {
        content: content.into(),
        translate: Box::new(translate),
    })
}

struct TranslatingOperation<'operation, 'translation, Payload> {
    inner: &'operation mut dyn Operation,
    translate: &'translation (dyn Fn(&Payload, &mut dyn Operation) + Send + Sync),
}

impl<Payload: 'static> Operation for TranslatingOperation<'_, '_, Payload> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        let translate = self.translate;
        self.inner.traverse(&mut |inner_operation| {
            operate(&mut TranslatingOperation {
                inner: inner_operation,
                translate,
            });
        });
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        self.inner.container(id, bounds);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        self.inner
            .scrollable(id, bounds, content_bounds, translation, state);
    }

    fn focusable(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Focusable) {
        self.inner.focusable(id, bounds, state);
    }

    fn text_input(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn TextInput) {
        self.inner.text_input(id, bounds, state);
    }

    fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.inner.text(id, bounds, text);
    }

    fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
        match state.downcast_ref::<Payload>() {
            Some(payload) => (self.translate)(payload, self.inner),
            None => self.inner.custom(id, bounds, state),
        }
    }
}

impl<Message, Payload: 'static> Widget<Message, Theme, iced::Renderer>
    for RegionTranslation<'_, Message, Payload>
{
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
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
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
            layout,
            renderer,
            &mut TranslatingOperation {
                inner: operation,
                translate: &*self.translate,
            },
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
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'overlay>(
        &'overlay mut self,
        tree: &'overlay mut Tree,
        layout: Layout<'overlay>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'overlay, Message, Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
