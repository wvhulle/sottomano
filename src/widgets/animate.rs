//! Self-driving animation: a transparent wrapper that keeps redraws firing
//! while an [`Animation`] is in progress, then goes quiet.
//!
//! iced's [`Animation`] is a *pure* curve — it holds no clock, so a smooth
//! transition needs a fresh redraw each frame to re-sample it. Rather than a
//! top-level `window::frames()` subscription that the application has to gate on
//! every group's animation state, this widget re-arms `shell.request_redraw()`
//! on each `RedrawRequested` for as long as its animation is animating. That is
//! the same mechanism iced's own `scrollable`/`slider` use for their internal
//! animations (and what the unreleased `iced::widget::transition` packages up),
//! so the whole expand animation stays encapsulated in the widget tree instead
//! of leaking a tick into the app.

use iced::{
    Animation, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    window,
};

/// Wrap `content` so that, while `animation` is in progress, the widget keeps
/// requesting the next frame — driving the animation to completion without a
/// top-level frame-tick. Once the curve settles, `is_animating` returns false
/// and no further redraws are requested, so an idle group costs nothing.
pub fn redraw_while<'a, Message: 'a>(
    animation: Animation<bool>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(RedrawWhile {
        animation,
        content: content.into(),
    })
}

struct RedrawWhile<'a, Message> {
    animation: Animation<bool>,
    content: Element<'a, Message>,
}

/// Transparent to layout and the tree — it adds no node of its own, deferring
/// `tag`/`state`/`children` to the child so `tree`/`layout` pass straight
/// through. The only behaviour it adds is re-arming the redraw in `update`.
impl<Message> Widget<Message, iced::Theme, iced::Renderer> for RedrawWhile<'_, Message> {
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

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && self.animation.is_animating(*now)
        {
            shell.request_redraw();
        }
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
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
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
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

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
