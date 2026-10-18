use core::slice;
use std::time::{Duration, Instant};

use iced::{
    Border, Color, Element, Event, Length, Padding, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::{Cursor, Interaction},
};

const OUTLINE_ALPHA: f32 = 0.45;
const COPIED_FILL_ALPHA: f32 = 0.3;
const BORDER_PX: f32 = 1.0;
const RADIUS_PX: f32 = 3.0;
const PADDING: Padding = Padding {
    top: 1.0,
    right: 4.0,
    bottom: 1.0,
    left: 4.0,
};

pub fn pressable_text<'a, Payload: Clone + 'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    payload: Payload,
    on_press: impl Fn(Payload) -> Message + 'a,
    accent: Color,
    hold: Duration,
    interaction: Interaction,
) -> Element<'a, Message> {
    Element::new(PressableText {
        content: content.into(),
        payload,
        on_press: Box::new(on_press),
        accent,
        hold,
        interaction,
    })
}

struct PressableText<'a, Payload, Message> {
    content: Element<'a, Message>,
    payload: Payload,
    on_press: Box<dyn Fn(Payload) -> Message + 'a>,
    accent: Color,
    hold: Duration,
    interaction: Interaction,
}

#[derive(Default)]
struct CellState {
    hovered: bool,
    copied_at: Option<Instant>,
}

impl CellState {
    fn holding(&self, now: Instant, hold: Duration) -> bool {
        self.copied_at
            .is_some_and(|at| now.duration_since(at) < hold)
    }
}

impl<Payload: Clone, Message> Widget<Message, iced::Theme, iced::Renderer>
    for PressableText<'_, Payload, Message>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CellState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CellState::default())
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
            &limits.shrink(PADDING),
        );
        let padded = child.size().expand(PADDING);
        Node::with_children(padded, vec![child.move_to((PADDING.left, PADDING.top))])
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
        let over = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<CellState>();
        if state.hovered != over {
            state.hovered = over;
            shell.request_redraw();
        }
        if shell.is_event_captured() || !over {
            return;
        }
        if !matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            return;
        }
        let now = Instant::now();
        state.copied_at = Some(now);
        shell.publish((self.on_press)(self.payload.clone()));
        shell.capture_event();
        shell.request_redraw();
        shell.request_redraw_at(now + self.hold);
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
        let state = tree.state.downcast_ref::<CellState>();
        let copied = state.holding(Instant::now(), self.hold);
        if copied || state.hovered {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        color: self
                            .accent
                            .scale_alpha(if copied { 1.0 } else { OUTLINE_ALPHA }),
                        width: BORDER_PX,
                        radius: Radius::new(RADIUS_PX),
                    },
                    ..renderer::Quad::default()
                },
                if copied {
                    self.accent.scale_alpha(COPIED_FILL_ALPHA)
                } else {
                    Color::TRANSPARENT
                },
            );
        }
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

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> Interaction {
        if cursor.is_over(layout.bounds()) {
            return self.interaction;
        }
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
