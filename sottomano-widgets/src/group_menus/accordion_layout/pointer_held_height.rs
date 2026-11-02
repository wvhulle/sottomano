use std::slice::from_ref;

use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

use crate::group_menus::cascading_layout::notched_panel::PanelGrowth;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct HeldHeight {
    tallest_px: f32,
    pointer_inside: bool,
}

impl HeldHeight {
    fn holding(self, natural_px: f32) -> f32 {
        if self.pointer_inside {
            natural_px.max(self.tallest_px)
        } else {
            natural_px
        }
    }

    fn after_layout(self, shown_px: f32) -> Self {
        Self {
            tallest_px: shown_px,
            ..self
        }
    }

    fn after_pointer(self, pointer_inside: bool) -> Self {
        Self {
            pointer_inside,
            tallest_px: if pointer_inside { self.tallest_px } else { 0.0 },
        }
    }
}

#[must_use]
pub fn pointer_held_height<'a, Message: 'a>(
    content: Element<'a, Message>,
    growth: PanelGrowth,
) -> Element<'a, Message> {
    Element::new(PointerHeldHeight { content, growth })
}

struct PointerHeldHeight<'a, Message> {
    content: Element<'a, Message>,
    growth: PanelGrowth,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for PointerHeldHeight<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HeldHeight>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HeldHeight::default())
    }

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
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let natural = child.size();
        let held = tree.state.downcast_mut::<HeldHeight>();
        let shown_px = held.holding(natural.height).min(limits.max().height);
        *held = held.after_layout(shown_px);
        let child_top = match self.growth {
            PanelGrowth::Downward => 0.0,
            PanelGrowth::Upward => shown_px - natural.height,
        };
        Node::with_children(
            Size::new(natural.width, shown_px),
            vec![child.move_to(Point::new(0.0, child_top))],
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
        let pointer_inside = cursor.position_over(layout.bounds()).is_some();
        let held = tree.state.downcast_mut::<HeldHeight>();
        if held.pointer_inside != pointer_inside {
            *held = held.after_pointer(pointer_inside);
            if !pointer_inside {
                shell.invalidate_layout();
            }
        }
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
    use super::HeldHeight;

    #[test]
    fn height_holds_only_while_the_pointer_is_inside() {
        let inside = HeldHeight::default()
            .after_pointer(true)
            .after_layout(300.0);
        assert!((inside.holding(100.0) - 300.0).abs() < f32::EPSILON);
        let outside = inside.after_pointer(false);
        assert!((outside.holding(100.0) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn growing_content_is_never_cut_while_held() {
        let inside = HeldHeight::default()
            .after_pointer(true)
            .after_layout(100.0);
        assert!((inside.holding(250.0) - 250.0).abs() < f32::EPSILON);
    }
}
