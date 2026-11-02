use std::slice::from_ref;

use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::Cursor,
};

use crate::widgets::reveal::clip_cursor;

const LINE_STEP_PX: f32 = 40.0;

#[must_use]
fn clamped_scroll(scrolled_px: f32, content_height: f32, visible_height: f32) -> f32 {
    scrolled_px.clamp(0.0, (content_height - visible_height).max(0.0))
}

pub fn menu_rows_viewport<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    scrolled_px: f32,
    on_scroll: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(MenuRowsViewport {
        content: content.into(),
        scrolled_px,
        on_scroll: Box::new(on_scroll),
    })
}

struct MenuRowsViewport<'a, Message> {
    content: Element<'a, Message>,
    scrolled_px: f32,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for MenuRowsViewport<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let visible_budget = limits.max().height;
        let natural_limits = Limits::new(Size::ZERO, Size::new(limits.max().width, f32::INFINITY));
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &natural_limits);
        let natural = child.size();
        let visible_height = natural.height.min(visible_budget);
        let scrolled = clamped_scroll(self.scrolled_px, natural.height, visible_height);
        Node::with_children(
            Size::new(natural.width, visible_height),
            vec![child.move_to(Point::new(0.0, -scrolled))],
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
        let Some(content_clip) = bounds.intersection(viewport) else {
            return;
        };
        renderer.with_layer(content_clip, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                clip_cursor(cursor, bounds),
                &content_clip,
            );
        });
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let child_layout = layout.children().next().unwrap();
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            child_layout,
            clip_cursor(cursor, bounds),
            renderer,
            clipboard,
            shell,
            &bounds,
        );
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && !shell.is_event_captured()
            && cursor.position_over(bounds).is_some()
        {
            let travel = match *delta {
                mouse::ScrollDelta::Lines { y, .. } => y * LINE_STEP_PX,
                mouse::ScrollDelta::Pixels { y, .. } => y,
            };
            let content_height = child_layout.bounds().height;
            let current = clamped_scroll(self.scrolled_px, content_height, bounds.height);
            let next = clamped_scroll(current - travel, content_height, bounds.height);
            if (next - current).abs() > f32::EPSILON {
                shell.publish((self.on_scroll)(next));
                shell.capture_event();
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            clip_cursor(cursor, bounds),
            &bounds,
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
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
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

#[cfg(test)]
mod tests {
    use super::clamped_scroll;

    #[test]
    fn scroll_is_clamped_to_the_overflow() {
        assert!((clamped_scroll(500.0, 300.0, 100.0) - 200.0).abs() < f32::EPSILON);
        assert!(clamped_scroll(-20.0, 300.0, 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn content_that_fits_never_scrolls() {
        assert!(clamped_scroll(50.0, 80.0, 100.0).abs() < f32::EPSILON);
    }
}
