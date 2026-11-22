mod card_slot_tracking;

use corredo::{
    transitions::MotionState,
    width_layout::{ChildView, ChildViewMut},
};
use iced::{
    Element, Event, Length, Padding, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use sottomano_toml_config::Config;

use self::card_slot_tracking::{ChildOrder, Drifts};
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

impl<'view> WindowAnchoredToasts<'view> {
    #[must_use]
    pub fn new(
        output: &str,
        toasts: Vec<AnchoredToast<'view>>,
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
                    corner_radius: Radius::from(overlays.frame.corner_radius_px),
                },
                ids: Vec::new(),
                settled_ids: Vec::new(),
                anchors: Vec::new(),
                cards: Vec::new(),
                background_fade,
                motion: card.motion,
                anchor_spacing: ToastAnchorSpacing::from(config),
                reveal_motion: MotionState::Settled,
            },
            |mut toasts, toast| {
                toasts.ids.push(toast.id);
                toasts.anchors.push(toast.anchor);
                toasts.cards.push(toast.card);
                match toast.motion {
                    MotionState::Moving => toasts.reveal_motion = MotionState::Moving,
                    MotionState::Settled => toasts.settled_ids.push(toast.id),
                }
                toasts
            },
        )
    }
}

impl<'view> From<WindowAnchoredToasts<'view>> for Element<'view, NotificationMessage> {
    fn from(toasts: WindowAnchoredToasts<'view>) -> Self {
        Element::new(toasts)
    }
}

pub struct WindowAnchoredToasts<'view> {
    output: String,
    panel_clearance: Padding,
    card_roles: RegionRoles,
    ids: Vec<u32>,
    settled_ids: Vec<u32>,
    anchors: Vec<ToastAnchor>,
    cards: Vec<Element<'view, NotificationMessage>>,
    background_fade: CardBackgroundFade,
    motion: Duration,
    anchor_spacing: ToastAnchorSpacing,
    reveal_motion: MotionState,
}

#[derive(Default)]
struct FieldState {
    order: ChildOrder,
    drift: Drifts,
    last_redraw: LastRedraw,
    published_cards: Vec<Rectangle>,
    reported_settled_ids: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum LastRedraw {
    #[default]
    NotYetRedrawn,
    RedrawnAt(Instant),
}

fn snapped(bounds: Rectangle) -> Rectangle {
    let left_edge = bounds.x.ceil();
    let top_edge = bounds.y.ceil();
    Rectangle {
        x: left_edge,
        y: top_edge,
        width: (bounds.x + bounds.width).floor() - left_edge,
        height: (bounds.y + bounds.height).floor() - top_edge,
    }
}

fn card_rects(layout: Layout<'_>) -> impl Iterator<Item = Rectangle> {
    layout
        .children()
        .map(|card_layout| snapped(card_layout.bounds()))
}

impl<'view> WindowAnchoredToasts<'view> {
    fn views<'tree>(
        &'tree self,
        tree: &'tree Tree,
        layout: Layout<'tree>,
    ) -> impl DoubleEndedIterator<Item = ChildView<'tree, 'view, NotificationMessage>> {
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

    fn views_mut<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
    ) -> impl DoubleEndedIterator<Item = ChildViewMut<'tree, 'view, NotificationMessage>> {
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
        state
            .reported_settled_ids
            .retain(|toast_id| self.ids.contains(toast_id));
        let newly_settled_ids: Vec<u32> = self
            .settled_ids
            .iter()
            .filter(|toast_id| !state.reported_settled_ids.contains(toast_id))
            .copied()
            .collect();
        state
            .reported_settled_ids
            .extend_from_slice(&newly_settled_ids);
        if !newly_settled_ids.is_empty() {
            shell.publish(NotificationMessage::CardsSettled {
                toast_ids: newly_settled_ids,
            });
        }
        if state.last_redraw == LastRedraw::RedrawnAt(*frame) {
            shell.request_redraw();
            return;
        }
        state.last_redraw = LastRedraw::RedrawnAt(*frame);
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

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, NotificationMessage, iced::Theme, iced::Renderer>> {
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
mod window_anchored_toasts_tests;
