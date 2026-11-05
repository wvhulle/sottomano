use iced::{
    Element, Event, Length, Rectangle, Size, Transformation, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

pub fn reveal_x<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    fraction: f32,
) -> Element<'a, Message> {
    reveal(content, fraction, Axis::Horizontal, 1.0)
}

pub fn reveal_y<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    fraction: f32,
    scale: f32,
) -> Element<'a, Message> {
    reveal(content, fraction, Axis::Vertical, scale)
}

fn reveal<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    fraction: f32,
    axis: Axis,
    scale: f32,
) -> Element<'a, Message> {
    Element::new(Reveal {
        content: content.into(),
        fraction: fraction.clamp(0.0, 1.0),
        axis,
        scale,
    })
}

struct Reveal<'a, Message> {
    content: Element<'a, Message>,
    fraction: f32,
    axis: Axis,
    scale: f32,
}

impl<Message> Reveal<'_, Message> {
    fn scaling_about_centre(&self, bounds: Rectangle) -> Transformation {
        let centre = Vector::new(bounds.center_x(), bounds.center_y());
        Transformation::translate(centre.x, centre.y)
            * Transformation::scale(self.scale)
            * Transformation::translate(-centre.x, -centre.y)
    }
}

#[must_use]
pub fn clip_cursor(cursor: Cursor, visible: Rectangle) -> Cursor {
    match cursor.position() {
        Some(position) if visible.contains(position) => cursor,
        _ => Cursor::Unavailable,
    }
}

fn cursor_unless_hidden(cursor: Cursor, visible: Rectangle, content: Rectangle) -> Cursor {
    match cursor.position() {
        Some(position) if content.contains(position) && !visible.contains(position) => {
            Cursor::Unavailable
        }
        _ => cursor,
    }
}

impl<Message> Reveal<'_, Message> {
    fn natural_limits(&self, limits: &Limits) -> Limits {
        let max = limits.max();
        match self.axis {
            Axis::Horizontal => Limits::new(Size::ZERO, Size::new(f32::INFINITY, max.height)),
            Axis::Vertical => Limits::new(Size::ZERO, Size::new(max.width, f32::INFINITY)),
        }
    }

    fn measures_finite(&self, natural: Size) -> bool {
        match self.axis {
            Axis::Horizontal => natural.width.is_finite(),
            Axis::Vertical => natural.height.is_finite(),
        }
    }

    fn revealed(&self, natural: Size) -> Size {
        match self.axis {
            Axis::Horizontal => Size::new(natural.width * self.fraction, natural.height),
            Axis::Vertical => Size::new(natural.width, natural.height * self.fraction),
        }
    }

    fn clip(&self, bounds: Rectangle, viewport: &Rectangle) -> Option<Rectangle> {
        if self.fraction >= 1.0 {
            return None;
        }
        Some(self.clipped_to(bounds, viewport))
    }

    fn clipped_to(&self, bounds: Rectangle, viewport: &Rectangle) -> Rectangle {
        match self.axis {
            Axis::Horizontal => Rectangle {
                x: bounds.x,
                width: bounds.width,
                y: viewport.y,
                height: viewport.height,
            },
            Axis::Vertical => Rectangle {
                x: viewport.x,
                width: viewport.width,
                y: bounds.y,
                height: bounds.height,
            },
        }
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Reveal<'_, Message> {
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
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let natural_limits = self.natural_limits(limits);
        let unbounded_child = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &natural_limits);
        let child = if self.measures_finite(unbounded_child.size()) {
            unbounded_child
        } else {
            self.content.as_widget_mut().layout(tree, renderer, limits)
        };
        let natural = child.size();
        Node::with_children(self.revealed(natural), vec![child])
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
        let child = layout.children().next().unwrap();
        let scaling = self.scaling_about_centre(child.bounds());
        let paint = |renderer: &mut iced::Renderer, visible: &Rectangle| {
            renderer.with_transformation(scaling, |renderer| {
                self.content
                    .as_widget()
                    .draw(tree, renderer, theme, style, child, cursor, visible);
            });
        };
        match self.clip(layout.bounds(), viewport) {
            Some(clip) => {
                if let Some(visible) = clip.intersection(viewport) {
                    renderer.with_layer(visible, |renderer| paint(renderer, &visible));
                }
            }
            None => paint(renderer, viewport),
        }
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
        let content = layout.children().next().unwrap();
        self.content.as_widget_mut().update(
            tree,
            event,
            content,
            cursor_unless_hidden(cursor, layout.bounds(), content.bounds()),
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
            tree,
            layout.children().next().unwrap(),
            clip_cursor(cursor, layout.bounds()),
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
        if self.fraction <= 0.0 {
            return None;
        }
        self.content.as_widget_mut().overlay(
            tree,
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::{
        Element, Settings, Size,
        widget::{row, rule, text},
    };
    use iced_test::Simulator;

    use super::reveal_x;

    const ROW_WIDTH: f32 = 400.0;
    const MARKER_AFTER_REVEAL: &str = "marker after the reveal";

    #[test]
    fn revealing_fill_width_content_keeps_a_finite_width() {
        let revealed: Element<'_, ()> = reveal_x(rule::horizontal(2.0), 1.0);
        let mut simulator = Simulator::with_size(
            Settings::default(),
            Size::new(ROW_WIDTH, 40.0),
            row![revealed, text(MARKER_AFTER_REVEAL)],
        );
        let marker = simulator
            .find(MARKER_AFTER_REVEAL)
            .expect("the marker is laid out")
            .bounds();
        assert!(
            marker.x.is_finite() && marker.x <= ROW_WIDTH,
            "the reveal pushed the marker to x = {}",
            marker.x
        );
    }
}
