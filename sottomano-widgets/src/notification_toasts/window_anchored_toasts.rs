use std::{collections::HashMap, convert::identity};

use corredo::{
    transitions::MotionState,
    width_layout::{ChildView, ChildViewMut},
};
use iced::{
    Animation, Element, Event, Length, Padding, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    border::Radius,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use sottomano_toml_config::Config;

use super::{
    toast_anchor_geometry,
    toast_anchor_geometry::ToastAnchorSpacing,
    toast_card::TOAST_CARD_WIDTH_PX,
    toast_placement::{AnchoredToast, ToastAnchor},
};
use crate::{
    desktop_panels::region_reporter::{PointerInput, RegionRoles},
    toast_messages::NotificationMessage,
    widgets::{shell_layer_appearance::LayerAppearance, typing_under_cards::CardBackgroundFade},
};

impl<'a> WindowAnchoredToasts<'a> {
    #[must_use]
    pub fn new(
        output: &str,
        toasts: Vec<AnchoredToast<'a>>,
        panel_clearance: Padding,
        overlays: &LayerAppearance,
        background_fade: CardBackgroundFade,
        config: &Config,
    ) -> Self {
        let card = &config.notification_toasts.card;
        toasts.into_iter().fold(
            Self {
                output: output.to_owned(),
                panel_clearance,
                card_roles: RegionRoles {
                    pointer_input: PointerInput::Accepted,
                    backdrop_blur: overlays.backdrop_blur,
                    corner_radius: Radius::from(overlays.corner_radius_px),
                },
                ids: Vec::new(),
                anchors: Vec::new(),
                cards: Vec::new(),
                background_fade,
                motion: card.motion,
                anchor_spacing: ToastAnchorSpacing::from_config(config),
                reveal_motion: MotionState::Settled,
            },
            |mut toasts, toast| {
                toasts.ids.push(toast.id);
                toasts.anchors.push(toast.anchor);
                toasts.cards.push(toast.card);
                if toast.motion == MotionState::Moving {
                    toasts.reveal_motion = MotionState::Moving;
                }
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
    output: String,
    panel_clearance: Padding,
    card_roles: RegionRoles,
    ids: Vec<u32>,
    anchors: Vec<ToastAnchor>,
    cards: Vec<Element<'a, NotificationMessage>>,
    background_fade: CardBackgroundFade,
    motion: Duration,
    anchor_spacing: ToastAnchorSpacing,
    reveal_motion: MotionState,
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
}

fn motion(value: f32, duration: Duration) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(duration)
}

#[derive(Default)]
struct FieldState {
    order: ChildOrder,
    drift: Drifts,
    frame: Option<Instant>,
    published_cards: Vec<Rectangle>,
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

    fn advance(&mut self, frame: Instant) -> MotionState {
        self.by_id
            .values_mut()
            .for_each(|drift| drift.advance(frame));
        if self
            .by_id
            .values()
            .any(|drift| drift.x.is_animating(frame) || drift.y.is_animating(frame))
        {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
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
        let card_limits = Limits::new(Size::ZERO, Size::new(TOAST_CARD_WIDTH_PX, f32::INFINITY));
        let nodes: Vec<Node> = self
            .cards
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(card, child)| card.as_widget_mut().layout(child, renderer, &card_limits))
            .collect();
        let sizes: Vec<Size> = nodes.iter().map(Node::size).collect();
        let free_area = Rectangle::with_size(surface).shrink(self.panel_clearance);
        let targets = toast_anchor_geometry::card_origins(
            &self.anchors,
            &sizes,
            free_area,
            &self.anchor_spacing,
        );
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
        let blurred_rects: Vec<Rectangle> = rects
            .iter()
            .copied()
            .filter(|rect| self.background_fade.fade_level_over(*rect) <= 0.0)
            .collect();
        self.card_roles.report_with_blurred_subset(
            &rects,
            &blurred_rects,
            layout.bounds(),
            operation,
        );
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
        let laid_out_cards: Vec<Rectangle> = card_rects(layout).collect();
        if state.published_cards != laid_out_cards {
            state.published_cards.clone_from(&laid_out_cards);
            shell.publish(NotificationMessage::CardsLaidOut {
                output: self.output.clone(),
                toast_cards: laid_out_cards,
            });
        }
        if state.frame == Some(*frame) {
            shell.request_redraw();
            return;
        }
        state.frame = Some(*frame);
        let drift_motion = state.drift.advance(*frame);
        if drift_motion == MotionState::Moving {
            shell.invalidate_layout();
        }
        if drift_motion == MotionState::Moving || self.reveal_motion == MotionState::Moving {
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

#[cfg(test)]
mod tests {
    use corredo::transitions::MotionState;
    use iced::{
        Event, Padding, Settings, Size,
        time::Duration,
        widget::{container, text},
        window,
    };
    use iced_test::Simulator;
    use sottomano_toml_config::{
        Config,
        screen_edges::{HEdge, VEdge},
    };

    use super::WindowAnchoredToasts;
    use crate::{
        clock::Clock,
        notification_toasts::toast_placement::{AnchoredToast, ToastAnchor},
        toast_messages::NotificationMessage,
        widgets::{shell_layer_appearance::ShellLayer, typing_under_cards::CardBackgroundFade},
    };

    #[test]
    fn unchanged_card_bounds_are_published_once() {
        let config = Config::default();
        let single_toast = AnchoredToast {
            id: 1,
            anchor: ToastAnchor::Edge {
                vertical: VEdge::Top,
                horizontal: HEdge::Right,
            },
            card: container(text("build finished")).padding(20).into(),
            motion: MotionState::Settled,
        };
        let toasts = WindowAnchoredToasts::new(
            "DP-1",
            vec![single_toast],
            Padding::ZERO,
            &ShellLayer::Overlays.default_appearance(),
            CardBackgroundFade::Resting,
            &config,
        );
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(1200.0, 800.0), toasts);
        let first_frame = Clock::real().instant;
        let _redrawn = simulator.simulate((0..3).map(|frame_index| {
            Event::Window(window::Event::RedrawRequested(
                first_frame + Duration::from_millis(16 * frame_index),
            ))
        }));
        let published_card_counts: Vec<usize> = simulator
            .into_messages()
            .filter_map(|message| match message {
                NotificationMessage::CardsLaidOut { toast_cards, .. } => Some(toast_cards.len()),
                _ => None,
            })
            .collect();
        assert_eq!(published_card_counts, [1]);
    }
}
