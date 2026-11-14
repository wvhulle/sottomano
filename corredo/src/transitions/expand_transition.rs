use std::slice;

use iced::{
    Animation, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    mouse::Cursor,
    time::{Duration, Instant},
    widget::space,
    window,
};

use super::motion_state::eased_animation;

const EXPANSION_EASING: Easing = Easing::EaseInOutCubic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpansionState {
    Expanded,
    #[default]
    Collapsed,
}

pub fn expand_transition<'a, Message: 'a>(
    expansion_state: ExpansionState,
    duration: Duration,
    view: impl Fn(f32) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
    Element::new(ExpandTransition {
        expansion_state,
        duration,
        view: Box::new(view),
        content: Element::new(space::horizontal().width(0.0)),
    })
}

struct State {
    animation: Animation<bool>,
    at: Option<Instant>,
}

impl State {
    fn fraction(&self) -> f32 {
        match self.at {
            Some(at) => self.animation.interpolate(0.0, 1.0, at),
            None if self.animation.value() => 1.0,
            None => 0.0,
        }
    }
}

struct ExpandTransition<'a, Message> {
    expansion_state: ExpansionState,
    duration: Duration,
    view: Box<dyn Fn(f32) -> Element<'a, Message> + 'a>,
    content: Element<'a, Message>,
}

impl<Message> ExpandTransition<'_, Message> {
    fn rebuild(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_ref::<State>();
        let fraction = state.fraction();
        self.content = (self.view)(fraction);
        tree.diff_children(slice::from_ref(&self.content));
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for ExpandTransition<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            animation: eased_animation(
                self.expansion_state == ExpansionState::Expanded,
                self.duration,
            )
            .easing(EXPANSION_EASING),
            at: None,
        })
    }

    fn children(&self) -> Vec<Tree> {
        Vec::new()
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.rebuild(tree);
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let size = child.size();
        Node::with_children(size, vec![child])
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State>();
            let laid_out_mid_animation =
                state.at.is_some_and(|at| state.animation.is_animating(at));
            state
                .animation
                .go_mut(self.expansion_state == ExpansionState::Expanded, *now);
            state.at = Some(*now);
            let animating = state.animation.is_animating(*now);
            if animating || laid_out_mid_animation {
                shell.invalidate_layout();
            }
            if animating {
                shell.request_redraw();
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
