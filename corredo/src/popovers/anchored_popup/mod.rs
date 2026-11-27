pub(super) mod anchored_popup_builder;
mod popup_placement_geometry;
pub(super) mod popup_rendering_options;

use iced::{
    Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::Cursor,
    time::Duration,
    window,
};

use self::{
    popup_placement_geometry::{ChildFraming, PointerPlacement},
    popup_rendering_options::{
        OutsidePress, OutsidePressCapture, PopupArrival, PopupFraming, PopupPointerInput,
        PopupRegions, PopupRendering,
    },
};
use crate::{
    orientation::Edge,
    popovers::{ConnectedSurface, ConnectorTriangle, RectangleMorph},
    theming::FrameStyle,
    transitions::MotionState,
};

pub struct AnchoredPopup<'view, Message, ShownIdentity = ()> {
    renderings: Vec<PopupRendering<'view, Message>>,
    anchor: Rectangle,
    side: Edge,
    gap_px: f32,
    edge_clearance: PopupEdgeClearance,
    leading_alignment_px: f32,
    morph_duration: Duration,
    shown: ShownIdentity,
    outside_press: OutsidePress<Message>,
    frame_over: Box<dyn Fn(Rectangle) -> FrameStyle + 'view>,
    corner_radius_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupEdgeClearance {
    FlushAgainstEdge,
    ShadowInsideSurface,
}

enum LaidOutIdentity<ShownIdentity> {
    NotYetLaidOut,
    LaidOut(ShownIdentity),
}

impl<ShownIdentity: PartialEq> LaidOutIdentity<ShownIdentity> {
    fn differs_from(&self, shown: &ShownIdentity) -> bool {
        match self {
            LaidOutIdentity::NotYetLaidOut => true,
            LaidOutIdentity::LaidOut(laid_out) => laid_out != shown,
        }
    }
}

struct MorphState<ShownIdentity> {
    laid_out_identity: LaidOutIdentity<ShownIdentity>,
    morph: RectangleMorph,
    connector: ConnectorTriangle,
}

impl<Message: Clone, ShownIdentity: Clone + PartialEq + 'static>
    Widget<Message, iced::Theme, iced::Renderer> for AnchoredPopup<'_, Message, ShownIdentity>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MorphState<ShownIdentity>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(MorphState::<ShownIdentity> {
            laid_out_identity: LaidOutIdentity::NotYetLaidOut,
            morph: RectangleMorph::default(),
            connector: ConnectorTriangle::between(
                self.anchor,
                0.0,
                Rectangle::default(),
                self.side.opposite(),
                self.corner_radius_px,
            ),
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.child_elements().into_iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let switched = tree
            .state
            .downcast_ref::<MorphState<ShownIdentity>>()
            .laid_out_identity
            .differs_from(&self.shown);
        tree.diff_children(&self.child_elements());
        if switched {
            self.renderings
                .iter()
                .zip(&mut tree.children)
                .filter(|(rendering, _)| rendering.arrival == PopupArrival::Incoming)
                .for_each(|(rendering, child)| *child = Tree::new(&rendering.element));
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let surface = limits.max();
        let room_along_docking = match self.side {
            Edge::Bottom => surface.height - self.dock_line(),
            Edge::Top => self.dock_line(),
            Edge::Left | Edge::Right => surface.height,
        } - 2.0 * self.gap_px;
        let rendering_limits = Limits::new(
            Size::ZERO,
            Size::new(surface.width, room_along_docking.max(0.0)),
        );
        let natural_nodes: Vec<Node> = self
            .renderings
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(rendering, child)| {
                rendering
                    .element
                    .as_widget_mut()
                    .layout(child, renderer, &rendering_limits)
            })
            .collect();
        let fitted_limits: Vec<Limits> = self
            .renderings
            .iter()
            .zip(&natural_nodes)
            .map(|(rendering, natural)| match rendering.framing {
                PopupFraming::WholeContent => rendering_limits,
                PopupFraming::LeadingSurface(orientation) => self.cascade_limits(
                    orientation,
                    rendering.framing.framed_bounds(natural).size(),
                    surface,
                ),
            })
            .collect();
        let nodes: Vec<Node> = self
            .renderings
            .iter_mut()
            .zip(&mut tree.children)
            .zip(natural_nodes.into_iter().zip(fitted_limits))
            .map(
                |((rendering, child), (natural, fitted))| match rendering.framing {
                    PopupFraming::WholeContent => natural,
                    PopupFraming::LeadingSurface(_) => rendering
                        .element
                        .as_widget_mut()
                        .layout(child, renderer, &fitted),
                },
            )
            .collect();

        let arriving = self
            .renderings
            .iter()
            .zip(&nodes)
            .find(|(rendering, _)| rendering.arrival == PopupArrival::Incoming)
            .map(|(rendering, node)| rendering.framing.framed_bounds(node).size());
        let at_anchor = self.resting(Size::ZERO, surface);

        let state: &mut MorphState<ShownIdentity> = tree.state.downcast_mut();
        if state.laid_out_identity.differs_from(&self.shown) {
            state.laid_out_identity = LaidOutIdentity::LaidOut(self.shown.clone());
            state.morph.begin(at_anchor);
        }
        let target = arriving.map_or(at_anchor, |size| self.resting(size, surface));
        let popup = state.morph.follow(target);
        state.connector = ConnectorTriangle::between(
            self.anchor,
            self.dock_line(),
            popup,
            self.side.opposite(),
            self.corner_radius_px,
        );

        let placed = self
            .renderings
            .iter()
            .zip(nodes)
            .map(|(rendering, node)| {
                let framed = rendering.framing.framed_bounds(&node);
                let framed_origin = self.content_origin(popup, framed.height);
                node.move_to(framed_origin - Vector::new(framed.x, framed.y))
            })
            .collect();
        Node::with_children(surface, placed)
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
        let state: &MorphState<ShownIdentity> = tree.state.downcast_ref();
        let bounds = layout.bounds();
        let origin = Vector::new(bounds.x, bounds.y);
        let box_bounds = state.morph.current + origin;
        if box_bounds.width <= 0.0 || box_bounds.height <= 0.0 {
            return;
        }
        let frame = (self.frame_over)(box_bounds);
        state.connector.translated(origin).draw_framed_surface(
            renderer,
            theme,
            ConnectedSurface {
                frame,
                bounds: box_bounds,
                corners: Radius::from(self.corner_radius_px),
                shadow: frame.lifted_drop_shadow(),
            },
        );
        let content_clip = if self.child_framing() == ChildFraming::LeadingChild
            && self.frame_motion(state) == MotionState::Settled
        {
            *viewport
        } else {
            box_bounds
        };
        let Some(clipped_viewport) = content_clip.intersection(viewport) else {
            return;
        };
        renderer.with_layer(clipped_viewport, |renderer| {
            self.views(tree, layout).for_each(|rendering| {
                rendering.widget.as_widget().draw(
                    rendering.state,
                    renderer,
                    theme,
                    style,
                    rendering.layout,
                    cursor,
                    &clipped_viewport,
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
        let state: &MorphState<ShownIdentity> = tree.state.downcast_ref();
        let frame = self.popup_frame(state, layout);
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        operation.custom(
            None,
            layout.bounds(),
            &mut PopupRegions {
                surface_rows: frame.surface_rows,
                popup_bounds: state.morph.current + origin,
                bounds: layout.bounds(),
                pointer_input: match frame.motion {
                    MotionState::Settled => PopupPointerInput::Accepted,
                    MotionState::Moving => PopupPointerInput::PassedThrough,
                },
                outside_press_capture: match (state.morph.motion_state(), &self.outside_press) {
                    (MotionState::Settled, OutsidePress::Dismisses { .. }) => {
                        OutsidePressCapture::Captured
                    }
                    (MotionState::Moving, _) | (_, OutsidePress::PassesThrough) => {
                        OutsidePressCapture::PassedThrough
                    }
                },
            },
        );
        self.views_mut(tree, layout).for_each(|rendering| {
            rendering.widget.as_widget_mut().operate(
                rendering.state,
                rendering.layout,
                renderer,
                operation,
            );
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
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state: &mut MorphState<ShownIdentity> = tree.state.downcast_mut();
            let motion_before_tick = state.morph.motion_state();
            state.morph.tick(*now, self.morph_duration);
            if motion_before_tick == MotionState::Moving {
                shell.invalidate_layout();
            }
            if state.morph.motion_state() == MotionState::Moving {
                shell.request_redraw();
            }
        }
        if let Some(rendering) = self.live_view(tree, layout) {
            rendering.widget.as_widget_mut().update(
                rendering.state,
                event,
                rendering.layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)))
            && !shell.is_event_captured()
            && tree
                .state
                .downcast_ref::<MorphState<ShownIdentity>>()
                .morph
                .motion_state()
                == MotionState::Settled
            && let OutsidePress::Dismisses {
                dismissal,
                reachable_below,
            } = &self.outside_press
            && let Some(pressed_at) = cursor.position()
            && self.pointer_placement(tree, layout, pressed_at) == PointerPlacement::OutsidePopup
            && !reachable_below.iter().any(|reachable| {
                (*reachable + Vector::new(layout.bounds().x, layout.bounds().y))
                    .contains(pressed_at)
            })
        {
            shell.publish(dismissal.clone());
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
        self.views(tree, layout)
            .map(|rendering| {
                rendering.widget.as_widget().mouse_interaction(
                    rendering.state,
                    rendering.layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .find(|interaction| *interaction != mouse::Interaction::None)
            .unwrap_or_else(|| {
                cursor
                    .position()
                    .filter(|pointer| {
                        self.pointer_placement(tree, layout, *pointer)
                            == PointerPlacement::OverPopup
                    })
                    .map_or_else(mouse::Interaction::default, |_| mouse::Interaction::Idle)
            })
    }
}
