use std::{collections::HashMap, convert::identity};

use iced::{
    Animation, Element, Event, Length, Padding, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use vetrata_config::{Config, notification_toasts::ToastPlacementConfig};
use vetrata_state::LayerAppearance;

use super::{
    toast_anchor_geometry,
    toast_placement::{AnchoredToast, ToastAnchor},
};
use crate::{
    toast_messages::NotificationMessage,
    widgets::{
        child_view::{ChildView, ChildViewMut},
        region_reporter::RegionRoles,
    },
};

impl<'a> WindowAnchoredToasts<'a> {
    #[must_use]
    pub fn new(
        toasts: Vec<AnchoredToast<'a>>,
        bar_clearance: Padding,
        overlays: &LayerAppearance,
        config: &Config,
    ) -> Self {
        let card = &config.notification_toasts.card;
        toasts.into_iter().fold(
            Self {
                bar_clearance,
                card_roles: RegionRoles {
                    takes_input: true,
                    blurred: overlays.blurred,
                    corner_radius: overlays.corner_radius_px,
                },
                ids: Vec::new(),
                anchors: Vec::new(),
                cards: Vec::new(),
                card_width_px: card.width,
                motion: card.motion,
                placement: config.notification_toasts.placement.clone(),
                revealing: false,
            },
            |mut toasts, toast| {
                toasts.ids.push(toast.id);
                toasts.anchors.push(toast.anchor);
                toasts.cards.push(toast.card);
                toasts.revealing |= toast.animating;
                toasts
            },
        )
    }
}

impl<'a> From<WindowAnchoredToasts<'a>> for Element<'a, NotificationMessage> {
    fn from(toasts: WindowAnchoredToasts<'a>) -> Self {
        Element::new(toasts)
    }
}

pub struct WindowAnchoredToasts<'a> {
    bar_clearance: Padding,
    card_roles: RegionRoles,
    ids: Vec<u32>,
    anchors: Vec<ToastAnchor>,
    cards: Vec<Element<'a, NotificationMessage>>,
    card_width_px: f32,
    motion: Duration,
    placement: ToastPlacementConfig,
    revealing: bool,
}

struct Drift {
    x: Animation<f32>,
    y: Animation<f32>,
    target: Point,
    at: Point,
}

impl Drift {
    fn resting_at(point: Point, duration: Duration) -> Self {
        Self {
            x: motion(point.x, duration),
            y: motion(point.y, duration),
            target: point,
            at: point,
        }
    }

    fn advance(&mut self, frame: Instant) {
        self.x.go_mut(self.target.x, frame);
        self.y.go_mut(self.target.y, frame);
        self.at = Point::new(
            self.x.interpolate_with(identity, frame),
            self.y.interpolate_with(identity, frame),
        );
    }

    fn is_animating(&self, frame: Instant) -> bool {
        self.x.is_animating(frame) || self.y.is_animating(frame)
    }
}

fn motion(value: f32, duration: Duration) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(duration)
}

const CLOCK_TICK: Duration = Duration::from_secs(1);

struct FieldState {
    order: ChildOrder,
    drift: Drifts,
    frame: Instant,
    next_clock_tick: Instant,
}

impl Default for FieldState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            order: ChildOrder::default(),
            drift: Drifts::default(),
            frame: now,
            next_clock_tick: now + CLOCK_TICK,
        }
    }
}

impl FieldState {
    fn advance_clock(&mut self, frame: Instant, shell: &mut Shell<'_, NotificationMessage>) {
        if frame >= self.next_clock_tick {
            self.next_clock_tick = frame + CLOCK_TICK;
            shell.publish(NotificationMessage::Tick);
        }
        shell.request_redraw_at(self.next_clock_tick);
    }
}

#[derive(Default)]
struct ChildOrder {
    keys: Vec<u32>,
}

impl ChildOrder {
    fn rekey(&mut self, ids: &[u32], children: &mut Vec<Tree>) {
        if self.keys == ids {
            return;
        }
        let mut retired: Vec<Option<Tree>> = children.drain(..).map(Some).collect();
        *children = ids
            .iter()
            .map(|id| {
                self.keys
                    .iter()
                    .position(|key| key == id)
                    .and_then(|index| retired[index].take())
                    .unwrap_or_else(Tree::empty)
            })
            .collect();
        self.keys.clear();
        self.keys.extend_from_slice(ids);
    }
}

#[derive(Default)]
struct Drifts {
    by_id: HashMap<u32, Drift>,
}

impl Drifts {
    fn aim(&mut self, ids: &[u32], targets: &[Point], motion: Duration) -> Vec<Point> {
        self.by_id.retain(|id, _| ids.contains(id));
        ids.iter()
            .zip(targets)
            .map(|(id, target)| {
                let drift = self
                    .by_id
                    .entry(*id)
                    .or_insert_with(|| Drift::resting_at(*target, motion));
                drift.target = *target;
                drift.at
            })
            .collect()
    }

    fn advance(&mut self, frame: Instant) -> bool {
        self.by_id
            .values_mut()
            .for_each(|drift| drift.advance(frame));
        self.by_id.values().any(|drift| drift.is_animating(frame))
    }
}

fn snapped(bounds: Rectangle) -> Rectangle {
    let x = bounds.x.ceil();
    let y = bounds.y.ceil();
    Rectangle {
        x,
        y,
        width: (bounds.x + bounds.width).floor() - x,
        height: (bounds.y + bounds.height).floor() - y,
    }
}

fn card_rects(layout: Layout<'_>) -> impl Iterator<Item = Rectangle> {
    layout
        .children()
        .map(|card_layout| snapped(card_layout.bounds()))
}

impl<'a> WindowAnchoredToasts<'a> {
    fn views<'t>(
        &'t self,
        tree: &'t Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = ChildView<'t, 'a, NotificationMessage>> {
        self.cards
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((widget, state), layout)| ChildView {
                widget,
                state,
                layout,
            })
    }

    fn views_mut<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = ChildViewMut<'t, 'a, NotificationMessage>> {
        self.cards
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((widget, state), layout)| ChildViewMut {
                widget,
                state,
                layout,
            })
    }
}

impl Widget<NotificationMessage, iced::Theme, iced::Renderer> for WindowAnchoredToasts<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<FieldState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(FieldState::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.cards.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.state
            .downcast_mut::<FieldState>()
            .order
            .rekey(&self.ids, &mut tree.children);
        tree.diff_children(&self.cards);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let surface = limits.max();
        let card_limits = Limits::new(Size::ZERO, Size::new(self.card_width_px, f32::INFINITY));
        let nodes: Vec<Node> = self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(card, child)| card.as_widget_mut().layout(child, renderer, &card_limits))
            .collect();
        let sizes: Vec<Size> = nodes.iter().map(Node::size).collect();
        let free_area = Rectangle::with_size(surface).shrink(self.bar_clearance);
        let targets =
            toast_anchor_geometry::origins(&self.anchors, &sizes, free_area, &self.placement);
        let positions =
            tree.state
                .downcast_mut::<FieldState>()
                .drift
                .aim(&self.ids, &targets, self.motion);
        Node::with_children(
            surface,
            nodes
                .into_iter()
                .zip(positions)
                .map(|(node, position)| node.move_to(position))
                .collect(),
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
        self.views(tree, layout).rev().for_each(|card| {
            renderer.with_layer(*viewport, |renderer| {
                card.widget.as_widget().draw(
                    card.state,
                    renderer,
                    theme,
                    style,
                    card.layout,
                    cursor,
                    viewport,
                );
            });
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let rects: Vec<Rectangle> = card_rects(layout).collect();
        self.card_roles.report(&rects, layout.bounds(), operation);
        self.views_mut(tree, layout).for_each(|card| {
            card.widget
                .as_widget_mut()
                .operate(card.state, card.layout, renderer, operation);
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, NotificationMessage>,
        viewport: &Rectangle,
    ) {
        self.views_mut(tree, layout).for_each(|card| {
            card.widget.as_widget_mut().update(
                card.state,
                event,
                card.layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        });

        let Event::Window(window::Event::RedrawRequested(frame)) = event else {
            return;
        };
        let state = tree.state.downcast_mut::<FieldState>();
        if !self.cards.is_empty() {
            state.advance_clock(*frame, shell);
        }
        if state.frame == *frame {
            shell.request_redraw();
            return;
        }
        state.frame = *frame;
        let drifting = state.drift.advance(*frame);
        if drifting {
            shell.invalidate_layout();
        }
        if drifting || self.revealing {
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
        self.views(tree, layout)
            .map(|card| {
                card.widget.as_widget().mouse_interaction(
                    card.state,
                    card.layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .find(|interaction| *interaction != mouse::Interaction::None)
            .unwrap_or_default()
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, NotificationMessage, iced::Theme, iced::Renderer>> {
        overlay::from_children(
            &mut self.cards,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
