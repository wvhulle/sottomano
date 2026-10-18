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

use crate::{config::Config, desktop_shell::monitor_surfaces::Position};

const BACKDROP_ALPHA: f32 = 0.2;

pub fn focus_indicator<'a, Message>(
    slots: impl Into<Element<'a, Message>>,
    active: Option<usize>,
    color: Color,
    config: &Config,
) -> FocusIndicator<'a, Message> {
    FocusIndicator {
        slots: slots.into(),
        active,
        color,
        motion: config.panel_bar.focus_indicator_motion,
        thickness: config.panel_bar.indicator_thickness,
        edge: Position::Bottom,
    }
}

pub struct FocusIndicator<'a, Message> {
    slots: Element<'a, Message>,
    active: Option<usize>,
    color: Color,
    motion: Duration,
    thickness: f32,
    edge: Position,
}

impl<Message> FocusIndicator<'_, Message> {
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

    fn travelled_slot(&self, slot: Rectangle, travelled: Travelled) -> Rectangle {
        if self.edge.is_vertical() {
            Rectangle {
                y: travelled.start,
                height: travelled.extent,
                ..slot
            }
        } else {
            Rectangle {
                x: travelled.start,
                width: travelled.extent,
                ..slot
            }
        }
    }

    fn travelled(&self, tree: &Tree, layout: Layout<'_>) -> Option<Travelled> {
        self.active_slot(layout)?;
        let travel = tree.state.downcast_ref::<State>().travel.as_ref()?;
        Some(travel.travelled(Instant::now()))
    }
}

fn edge_band(edge: Position, slots: Rectangle, slot: Rectangle, thickness: f32) -> Rectangle {
    match edge {
        Position::Bottom => Rectangle {
            height: thickness,
            ..slot
        },
        Position::Top => Rectangle {
            y: slot.y + slot.height - thickness,
            height: thickness,
            ..slot
        },
        Position::Left => Rectangle {
            x: slots.x + slots.width - thickness,
            width: thickness,
            ..slot
        },
    }
}

fn edge_backdrop(edge: Position, slots: Rectangle, slot: Rectangle) -> Rectangle {
    if edge.is_vertical() {
        Rectangle {
            x: slots.x,
            width: slots.width,
            ..slot
        }
    } else {
        Rectangle {
            y: slots.y,
            height: slots.height,
            ..slot
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

fn motion(value: f32, duration: Duration) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(duration)
}

impl Travel {
    fn resting_at(span: Travelled, duration: Duration) -> Self {
        Self {
            start: motion(span.start, duration),
            extent: motion(span.extent, duration),
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
        let slots = layout.bounds();
        let travelled_slot = self
            .travelled(tree, layout)
            .and_then(|travelled| Some(self.travelled_slot(self.active_slot(layout)?, travelled)));
        if let Some(slot) = travelled_slot {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: edge_backdrop(self.edge, slots, slot),
                    snap: false,
                    ..renderer::Quad::default()
                },
                self.color.scale_alpha(BACKDROP_ALPHA),
            );
        }
        self.slots.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        let Some(slot) = travelled_slot else {
            return;
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: edge_band(self.edge, slots, slot, self.thickness),
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
        let travel = state
            .travel
            .get_or_insert_with(|| Travel::resting_at(span, self.motion));
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
