use core::slice;
use std::time::{Duration, Instant};

use iced::{
    Border, Color, Element, Event, Length, Rectangle, Size, Theme,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::{Cursor, Interaction},
    widget::container,
    window,
};

use crate::widgets::{
    configured_theme::AccentShades,
    shared_metrics::{OUTLINE_WIDTH_PX, PRESSABLE_OUTLINE_PADDING, SMALL_CORNER_RADIUS_PX},
};

pub fn pressable_text<'a, Payload: Clone + 'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    payload: Payload,
    on_press: impl Fn(Payload) -> Message + 'a,
    accent: fn(&Theme) -> AccentShades,
    hold: Duration,
    interaction: Interaction,
) -> Element<'a, Message> {
    Element::new(PressableText {
        content: container(content).padding(PRESSABLE_OUTLINE_PADDING).into(),
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
    accent: fn(&Theme) -> AccentShades,
    hold: Duration,
    interaction: Interaction,
}

#[derive(Default)]
enum Press {
    #[default]
    Idle,
    Pending,
    Held {
        since: Instant,
    },
}

#[derive(Default)]
struct CellState {
    hovered: bool,
    press: Press,
    frame: Option<Instant>,
}

impl CellState {
    fn holding(&self, hold: Duration) -> bool {
        match (&self.press, self.frame) {
            (Press::Held { since }, Some(frame)) => frame.duration_since(*since) < hold,
            _ => false,
        }
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
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            state.frame = Some(*now);
            if matches!(state.press, Press::Pending) {
                state.press = Press::Held { since: *now };
                shell.request_redraw_at(*now + self.hold);
            }
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
        state.press = Press::Pending;
        shell.publish((self.on_press)(self.payload.clone()));
        shell.capture_event();
        shell.request_redraw();
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
        let state = tree.state.downcast_ref::<CellState>();
        let copied = state.holding(self.hold);
        if copied || state.hovered {
            let accent = (self.accent)(theme);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        color: if copied { accent.base } else { accent.weak },
                        width: OUTLINE_WIDTH_PX,
                        radius: Radius::new(SMALL_CORNER_RADIUS_PX),
                    },
                    ..renderer::Quad::default()
                },
                if copied {
                    accent.weak
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
