mod glyph_scramble_hashing;
mod text_transition_state;
mod transition_effect_drawing;

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
    slice,
};

use iced::{
    Color, Element, Event, Font, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    widget::text,
    window,
};

use self::{
    text_transition_state::{ShownText, State, TransitionKind},
    transition_effect_drawing::TextColors,
};
use crate::{
    changing_text::TextEffects,
    theming::{TextFace, ThemedColor, primary_color, text_color},
    transitions::MotionState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPresence {
    Placeholder,
    Shown,
}

#[derive(Debug, Clone, Copy)]
pub struct TransitionStyle {
    pub effects: TextEffects,
    pub presence: TextPresence,
    pub highlight: fn(&iced::Theme) -> Color,
}

pub struct TransitioningText<'view, Message> {
    settled: Element<'view, Message>,
    text: String,
    identity: u64,
    style: TransitionStyle,
    font: Font,
    size: f32,
    color: ThemedColor<'view>,
}

impl<'view, Message> TransitioningText<'view, Message> {
    pub fn new(
        settled: impl Into<Element<'view, Message>>,
        text: String,
        style: TransitionStyle,
        face: TextFace,
        color: impl Fn(&iced::Theme) -> Color + 'view,
    ) -> Self {
        Self {
            settled: settled.into(),
            text,
            identity: 0,
            style,
            font: face.font,
            size: face.size_px,
            color: Box::new(color),
        }
    }

    #[must_use]
    pub fn identified_by(self, identity: &impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        identity.hash(&mut hasher);
        Self {
            identity: hasher.finish(),
            ..self
        }
    }
}

pub struct AnimatedText<'view> {
    text: String,
    identity: u64,
    style: TransitionStyle,
    face: TextFace,
    color: Rc<dyn Fn(&iced::Theme) -> Color + 'view>,
}

pub fn animated_text<'view>(text: impl Into<String>) -> AnimatedText<'view> {
    AnimatedText {
        text: text.into(),
        identity: 0,
        style: TransitionStyle {
            effects: TextEffects::default(),
            presence: TextPresence::Shown,
            highlight: primary_color,
        },
        face: TextFace::default(),
        color: Rc::new(text_color),
    }
}

impl<'view> AnimatedText<'view> {
    #[must_use]
    pub fn effects(mut self, effects: TextEffects) -> Self {
        self.style.effects = effects;
        self
    }

    #[must_use]
    pub fn highlight(mut self, highlight: fn(&iced::Theme) -> Color) -> Self {
        self.style.highlight = highlight;
        self
    }

    #[must_use]
    pub fn face(self, face: TextFace) -> Self {
        Self { face, ..self }
    }

    #[must_use]
    pub fn color(self, color: impl Fn(&iced::Theme) -> Color + 'view) -> Self {
        Self {
            color: Rc::new(color),
            ..self
        }
    }

    #[must_use]
    pub fn identified_by(self, identity: &impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        identity.hash(&mut hasher);
        Self {
            identity: hasher.finish(),
            ..self
        }
    }
}

impl<'view, Message: 'view> From<AnimatedText<'view>> for Element<'view, Message> {
    fn from(animated: AnimatedText<'view>) -> Self {
        let settled_color = Rc::clone(&animated.color);
        let settled = animated
            .face
            .text(animated.text.clone())
            .style(move |theme| text::Style {
                color: Some(settled_color(theme)),
            });
        let transition_color = animated.color;
        TransitioningText::new(
            settled,
            animated.text,
            animated.style,
            animated.face,
            move |theme| transition_color(theme),
        )
        .identified_by(&animated.identity)
        .into()
    }
}

impl<'view, Message: 'view> From<TransitioningText<'view, Message>> for Element<'view, Message> {
    fn from(text: TransitioningText<'view, Message>) -> Self {
        Element::new(text)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for TransitioningText<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::settled(self.shown_text()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.settled)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.state.downcast_mut::<State>().expect(self.shown_text());
        tree.diff_children(slice::from_ref(&self.settled));
    }

    fn size(&self) -> Size<Length> {
        self.settled.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child = self
            .settled
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
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
        let state = tree.state.downcast_ref::<State>();
        let settled = layout.children().next().unwrap();
        let area = settled.bounds();
        let text_colors = TextColors {
            text: (self.color)(theme),
            highlight: (self.style.highlight)(theme),
        };
        match &state.shown {
            ShownText::Placeholder => {
                self.draw_placeholder(
                    renderer,
                    area,
                    state.placeholder_elapsed(),
                    text_colors.text,
                );
            }
            ShownText::Text { .. } => match self.running(state) {
                None => self.settled.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    settled,
                    cursor,
                    viewport,
                ),
                Some(running) => match running.kind {
                    TransitionKind::Intro => {
                        self.draw_intro(renderer, area, &running, text_colors.text);
                    }
                    TransitionKind::Replace { from } => {
                        self.draw_replace(renderer, area, from, &running, text_colors);
                    }
                },
            },
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.settled.as_widget_mut().operate(
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
            state.frame(*now);
            if self.text_motion(state) == MotionState::Moving {
                shell.request_redraw();
            }
        }
        self.settled.as_widget_mut().update(
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
        self.settled.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, Message, iced::Theme, iced::Renderer>> {
        self.settled.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
