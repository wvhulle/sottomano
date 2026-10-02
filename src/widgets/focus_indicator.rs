use std::{convert::identity, slice::from_ref};

use iced::{
    Animation, Color, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    border,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};

use crate::desktop_shell::monitor_surfaces::Position;

const MOTION: Duration = Duration::from_millis(160);

pub fn focus_indicator<'a, Message>(
    slots: impl Into<Element<'a, Message>>,
    active: Option<usize>,
    color: Color,
) -> FocusIndicator<'a, Message> {
    FocusIndicator {
        slots: slots.into(),
        active,
        color,
        thickness: 1.0,
        edge: Position::Bottom,
    }
}

pub struct FocusIndicator<'a, Message> {
    slots: Element<'a, Message>,
    active: Option<usize>,
    color: Color,
    thickness: f32,
    edge: Position,
}

impl<Message> FocusIndicator<'_, Message> {
    #[must_use]
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    #[must_use]
    pub fn edge(mut self, edge: Position) -> Self {
        self.edge = edge;
        self
    }

    fn active_slot(&self, layout: Layout<'_>) -> Option<Rectangle> {
        Some(layout.children().nth(self.active?)?.bounds())
    }

    fn span(&self, slot: Rectangle) -> Travelled {
        if self.edge.is_vertical() {
            Travelled {
                start: slot.y,
                extent: slot.height,
            }
        } else {
            Travelled {
                start: slot.x,
                extent: slot.width,
            }
        }
    }

    fn band(&self, slots: Rectangle, slot: Rectangle, travelled: Travelled) -> Rectangle {
        match self.edge {
            Position::Bottom => Rectangle {
                x: travelled.start,
                y: slot.y,
                width: travelled.extent,
                height: self.thickness,
            },
            Position::Top => Rectangle {
                x: travelled.start,
                y: slot.y + slot.height - self.thickness,
                width: travelled.extent,
                height: self.thickness,
            },
            Position::Left => Rectangle {
                x: slots.x + slots.width - self.thickness,
                y: travelled.start,
                width: self.thickness,
                height: travelled.extent,
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Travelled {
    start: f32,
    extent: f32,
}

struct Travel {
    start: Animation<f32>,
    extent: Animation<f32>,
}

fn motion(value: f32) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(MOTION)
}

impl Travel {
    fn resting_at(span: Travelled) -> Self {
        Self {
            start: motion(span.start),
            extent: motion(span.extent),
        }
    }

    fn aim(&mut self, span: Travelled, now: Instant) {
        self.start.go_mut(span.start, now);
        self.extent.go_mut(span.extent, now);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.start.is_animating(now) || self.extent.is_animating(now)
    }

    fn travelled(&self, now: Instant) -> Travelled {
        Travelled {
            start: self.start.interpolate_with(identity, now),
            extent: self.extent.interpolate_with(identity, now),
        }
    }
}

#[derive(Default)]
struct State {
    travel: Option<Travel>,
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for FocusIndicator<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.slots)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.slots));
    }

    fn size(&self) -> Size<Length> {
        self.slots.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.slots
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
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.slots.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        let Some(slot) = self.active_slot(layout) else {
            return;
        };
        let Some(travel) = tree.state.downcast_ref::<State>().travel.as_ref() else {
            return;
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: self.band(layout.bounds(), slot, travel.travelled(Instant::now())),
                border: border::rounded(self.thickness / 2.0),
                snap: false,
                ..renderer::Quad::default()
            },
            self.color,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.slots
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        self.slots.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let Some(slot) = self.active_slot(layout) else {
            return;
        };
        let span = self.span(slot);
        let state = tree.state.downcast_mut::<State>();
        let travel = state.travel.get_or_insert_with(|| Travel::resting_at(span));
        travel.aim(span, *now);
        if travel.is_animating(*now) {
            shell.request_redraw();
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
        self.slots.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
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
        self.slots.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<FocusIndicator<'a, Message>> for Element<'a, Message> {
    fn from(indicator: FocusIndicator<'a, Message>) -> Self {
        Self::new(indicator)
    }
}
