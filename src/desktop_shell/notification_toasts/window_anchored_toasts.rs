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
    window::{self, Id},
};

use super::{
    toast_anchor_geometry,
    toast_lifecycle::NotificationMessage,
    toast_placement::{AnchoredToast, ToastAnchor},
};
use crate::config::ToastConfig;

const MOTION: Duration = Duration::from_millis(160);

impl<'a> WindowAnchoredToasts<'a> {
    pub fn new(
        window: Id,
        toasts: Vec<AnchoredToast<'a>>,
        bar_clearance: Padding,
        config: &ToastConfig,
    ) -> Self {
        toasts.into_iter().fold(
            Self {
                window,
                bar_clearance,
                ids: Vec::new(),
                anchors: Vec::new(),
                cards: Vec::new(),
                config: config.clone(),
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
    window: Id,
    bar_clearance: Padding,
    ids: Vec<u32>,
    anchors: Vec<ToastAnchor>,
    cards: Vec<Element<'a, NotificationMessage>>,
    config: ToastConfig,
    revealing: bool,
}

struct Drift {
    x: Animation<f32>,
    y: Animation<f32>,
    target: Point,
    at: Point,
}

impl Drift {
    fn resting_at(point: Point) -> Self {
        Self {
            x: motion(point.x),
            y: motion(point.y),
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

fn motion(value: f32) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(MOTION)
}

struct FieldState {
    order: ChildOrder,
    drift: Drifts,
    region: PublishedRects,
    frame: Instant,
}

impl Default for FieldState {
    fn default() -> Self {
        Self {
            order: ChildOrder::default(),
            drift: Drifts::default(),
            region: PublishedRects::default(),
            frame: Instant::now(),
        }
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
    fn aim(&mut self, ids: &[u32], targets: &[Point]) -> Vec<Point> {
        self.by_id.retain(|id, _| ids.contains(id));
        ids.iter()
            .zip(targets)
            .map(|(id, target)| {
                let drift = self
                    .by_id
                    .entry(*id)
                    .or_insert_with(|| Drift::resting_at(*target));
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

#[derive(Default)]
struct PublishedRects {
    published: Option<Vec<Rectangle>>,
}

impl PublishedRects {
    fn changed(&mut self, rects: Vec<Rectangle>) -> Option<Vec<Rectangle>> {
        if self.published.as_ref() == Some(&rects) {
            return None;
        }
        self.published = Some(rects.clone());
        Some(rects)
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

struct CardView<'t, 'a> {
    widget: &'t Element<'a, NotificationMessage>,
    state: &'t Tree,
    layout: Layout<'t>,
}

struct CardViewMut<'t, 'a> {
    widget: &'t mut Element<'a, NotificationMessage>,
    state: &'t mut Tree,
    layout: Layout<'t>,
}

impl<'a> WindowAnchoredToasts<'a> {
    fn views<'t>(
        &'t self,
        tree: &'t Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = CardView<'t, 'a>> {
        self.cards
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((widget, state), layout)| CardView {
                widget,
                state,
                layout,
            })
    }

    fn views_mut<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = CardViewMut<'t, 'a>> {
        self.cards
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((widget, state), layout)| CardViewMut {
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
        let card_limits = Limits::new(
            Size::ZERO,
            Size::new(self.config.card_size.card_width_px, f32::INFINITY),
        );
        let nodes: Vec<Node> = self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(card, child)| card.as_widget_mut().layout(child, renderer, &card_limits))
            .collect();
        let sizes: Vec<Size> = nodes.iter().map(Node::size).collect();
        let free_area = Rectangle::with_size(surface).shrink(self.bar_clearance);
        let targets = toast_anchor_geometry::origins(
            &self.anchors,
            &sizes,
            free_area,
            &self.config.placement,
        );
        let positions = tree
            .state
            .downcast_mut::<FieldState>()
            .drift
            .aim(&self.ids, &targets);
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
        let window = self.window;
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
        if state.frame == *frame {
            shell.request_redraw();
            return;
        }
        state.frame = *frame;
        let drifting = state.drift.advance(*frame);

        let rects = layout
            .children()
            .map(|card_layout| snapped(card_layout.bounds()))
            .collect();
        if let Some(rects) = state.region.changed(rects) {
            shell.publish(NotificationMessage::CardBoundsChanged { window, rects });
            shell.request_redraw();
        }
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
