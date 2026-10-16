use std::slice::from_ref;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    window::Id,
};

use crate::desktop_shell::{
    Message,
    hover_tooltips::{HoverTarget, PressHints, TooltipMessage},
    monitor_surfaces::Position,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    Hover,
    Press,
}

pub fn hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    press_hints: PressHints,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        press_hints,
        opening: Opening::Hover,
        wrap: Box::new(wrap),
    })
}

pub fn pressable_card<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        press_hints: PressHints::default(),
        opening: Opening::Press,
        wrap: Box::new(wrap),
    })
}

pub fn on_surface(
    surface: Id,
    position: Position,
    origin_in_overlay: Vector,
    content: Element<'_, Message>,
) -> Element<'_, Message> {
    content.map(move |message| match message {
        Message::Tooltip(TooltipMessage::EnterAnchor {
            target,
            anchor,
            press_hints,
        }) => Message::Tooltip(TooltipMessage::Enter {
            surface,
            position,
            target,
            anchor: anchor + origin_in_overlay,
            press_hints,
        }),
        other => other,
    })
}

struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    press_hints: PressHints,
    opening: Opening,
    wrap: Box<dyn Fn(TooltipMessage) -> M + 'a>,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
    opened_by_press: bool,
    announced: PressHints,
}

impl<M> HoverArea<'_, M> {
    fn enter(&self, state: &mut HoverState, anchor: Rectangle) -> M {
        state.announced = self.press_hints;
        (self.wrap)(TooltipMessage::EnterAnchor {
            target: self.target.clone(),
            anchor,
            press_hints: self.press_hints,
        })
    }

    fn exit(&self) -> M {
        (self.wrap)(TooltipMessage::Exit {
            target: self.target.clone(),
        })
    }

    fn revised_hints(&self, state: &mut HoverState) -> Option<M> {
        if !state.hovered || state.announced == self.press_hints {
            return None;
        }
        state.announced = self.press_hints;
        Some((self.wrap)(TooltipMessage::PressHintsChanged {
            target: self.target.clone(),
            press_hints: self.press_hints,
        }))
    }

    fn on_hover_change(
        &self,
        state: &mut HoverState,
        hovered: bool,
        anchor: Rectangle,
    ) -> Option<M> {
        if hovered == state.hovered {
            return None;
        }
        state.hovered = hovered;
        match self.opening {
            Opening::Hover if hovered => Some(self.enter(state, anchor)),
            Opening::Hover => Some(self.exit()),
            Opening::Press if hovered => None,
            Opening::Press => state.opened_by_press.then(|| {
                state.opened_by_press = false;
                self.exit()
            }),
        }
    }

    fn on_press(&self, state: &mut HoverState, anchor: Rectangle) -> Option<M> {
        if self.opening != Opening::Press || !state.hovered {
            return None;
        }
        state.opened_by_press = !state.opened_by_press;
        Some(if state.opened_by_press {
            self.enter(state, anchor)
        } else {
            self.exit()
        })
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for HoverArea<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::default())
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
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state: &mut HoverState = tree.state.downcast_mut();
        let anchor = layout.bounds();
        let hovered = cursor.is_over(anchor);
        let published = match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.on_press(state, anchor)
            }
            _ => self.on_hover_change(state, hovered, anchor),
        }
        .or_else(|| self.revised_hints(state));
        if let Some(message) = published {
            shell.publish(message);
        }
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
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
