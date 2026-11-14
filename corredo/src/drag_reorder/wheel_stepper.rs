use std::slice::from_ref;

use iced::{
    Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

use crate::selection_scroll::wheel_step_direction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WheelStep {
    Backward,
    Forward,
}

type StepMessage<'a, Message> = Box<dyn Fn(WheelStep) -> Option<Message> + 'a>;

pub(crate) struct WheelStepper<'a, Message> {
    content: Element<'a, Message>,
    on_step: StepMessage<'a, Message>,
}

impl<'a, Message> WheelStepper<'a, Message> {
    pub(crate) fn new(
        content: Element<'a, Message>,
        on_step: impl Fn(WheelStep) -> Option<Message> + 'a,
    ) -> Self {
        Self {
            content,
            on_step: Box::new(on_step),
        }
    }
}

#[derive(Default)]
struct AccumulatedWheel {
    scrolled_px: f32,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for WheelStepper<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<AccumulatedWheel>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(AccumulatedWheel::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(content.size(), vec![content])
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            content_layout(layout),
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
            content_layout(layout),
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
            content_layout(layout),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if shell.is_event_captured() || !cursor.is_over(layout.bounds()) {
            return;
        }
        let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event else {
            return;
        };
        let accumulated = tree.state.downcast_mut::<AccumulatedWheel>();
        let Some(steps) = wheel_step_direction(&mut accumulated.scrolled_px, *delta) else {
            return;
        };
        let step = if steps > 0 {
            WheelStep::Backward
        } else {
            WheelStep::Forward
        };
        if let Some(message) = (self.on_step)(step) {
            shell.publish(message);
            shell.capture_event();
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
            content_layout(layout),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'overlay>(
        &'overlay mut self,
        tree: &'overlay mut Tree,
        layout: Layout<'overlay>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'overlay, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout
                .children()
                .next()
                .expect("a wheel stepper lays out its content"),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<WheelStepper<'a, Message>> for Element<'a, Message> {
    fn from(wheel_stepper: WheelStepper<'a, Message>) -> Self {
        Element::new(wheel_stepper)
    }
}

fn content_layout(layout: Layout<'_>) -> Layout<'_> {
    layout
        .children()
        .next()
        .expect("a wheel stepper lays out its content")
}
