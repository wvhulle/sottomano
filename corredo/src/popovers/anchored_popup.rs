use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
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

use crate::{
    orientation::Edge,
    popovers::{
        CascadeDirection, CascadeOrientation, ConnectedSurface, ConnectorTriangle, PopupGrowth,
        RectangleMorph,
    },
    theming::FrameStyle,
    transitions::MotionState,
    width_layout::{ChildView, ChildViewMut},
};

const DEFAULT_GAP_PX: f32 = 8.0;
const DEFAULT_MORPH_DURATION: Duration = Duration::from_millis(160);

#[derive(Debug, Clone)]
pub struct PopupRegions {
    pub surface_rows: Vec<Rectangle>,
    pub popup_bounds: Rectangle,
    pub bounds: Rectangle,
    pub takes_input: bool,
    pub captures_outside_presses: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildFraming {
    LeadingChild,
    WholePopup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerPlacement {
    OverPopup,
    OutsidePopup,
}

struct PopupFrame {
    surface_rows: Vec<Rectangle>,
    motion: MotionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupFraming {
    WholeContent,
    LeadingSurface(CascadeOrientation),
}

impl PopupFraming {
    fn framed_bounds(self, node: &Node) -> Rectangle {
        let whole = Rectangle::new(Point::ORIGIN, node.size());
        match self {
            PopupFraming::WholeContent => whole,
            PopupFraming::LeadingSurface(_) => node.children().first().map_or(whole, Node::bounds),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupArrival {
    Incoming,
    Outgoing,
}

pub struct PopupRendering<'a, Message> {
    element: Element<'a, Message>,
    pub arrival: PopupArrival,
    pub framing: PopupFraming,
}

impl<'a, Message> PopupRendering<'a, Message> {
    #[must_use]
    pub fn incoming(element: Element<'a, Message>) -> Self {
        Self {
            element,
            arrival: PopupArrival::Incoming,
            framing: PopupFraming::WholeContent,
        }
    }

    #[must_use]
    pub fn outgoing(element: Element<'a, Message>) -> Self {
        Self {
            element,
            arrival: PopupArrival::Outgoing,
            framing: PopupFraming::WholeContent,
        }
    }

    #[must_use]
    pub fn framing(self, framing: PopupFraming) -> Self {
        Self { framing, ..self }
    }
}

pub enum OutsidePress<Message> {
    PassesThrough,
    Dismisses {
        dismissal: Message,
        reachable_below: Vec<Rectangle>,
    },
}

pub struct AnchoredPopup<'a, Message, ShownIdentity = ()> {
    renderings: Vec<PopupRendering<'a, Message>>,
    anchor: Rectangle,
    side: Edge,
    gap_px: f32,
    leading_alignment_px: f32,
    morph_duration: Duration,
    shown: Option<ShownIdentity>,
    outside_press: OutsidePress<Message>,
    frame_over: Box<dyn Fn(Rectangle) -> FrameStyle + 'a>,
    corner_radius_px: f32,
}

struct MorphState<ShownIdentity> {
    shown: Option<ShownIdentity>,
    morph: RectangleMorph,
    connector: ConnectorTriangle,
}

#[must_use]
pub fn anchored_popup<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    anchor: Rectangle,
) -> AnchoredPopup<'a, Message> {
    AnchoredPopup::with_renderings(vec![PopupRendering::incoming(content.into())], anchor)
}

impl<'a, Message: 'a> AnchoredPopup<'a, Message> {
    #[must_use]
    pub fn with_renderings(
        renderings: Vec<PopupRendering<'a, Message>>,
        anchor: Rectangle,
    ) -> Self {
        let frame = FrameStyle::default();
        Self {
            renderings,
            anchor,
            side: Edge::Bottom,
            gap_px: DEFAULT_GAP_PX,
            leading_alignment_px: 0.0,
            morph_duration: DEFAULT_MORPH_DURATION,
            shown: Some(()),
            outside_press: OutsidePress::PassesThrough,
            frame_over: Box::new(move |_| frame),
            corner_radius_px: frame.corner_radius_px,
        }
    }
}

impl<'a, Message: 'a, ShownIdentity> AnchoredPopup<'a, Message, ShownIdentity> {
    #[must_use]
    pub fn side(self, side: Edge) -> Self {
        Self { side, ..self }
    }

    pub(crate) fn set_anchor(&mut self, anchor: Rectangle) {
        self.anchor = anchor;
    }

    #[must_use]
    pub fn gap(self, gap_px: f32) -> Self {
        Self { gap_px, ..self }
    }

    #[must_use]
    pub fn morph_duration(self, morph_duration: Duration) -> Self {
        Self {
            morph_duration,
            ..self
        }
    }

    #[must_use]
    pub fn frame(self, frame: FrameStyle) -> Self {
        Self {
            frame_over: Box::new(move |_| frame),
            corner_radius_px: frame.corner_radius_px,
            ..self
        }
    }

    #[must_use]
    pub fn frame_over(self, frame_over: impl Fn(Rectangle) -> FrameStyle + 'a) -> Self {
        let corner_radius_px = frame_over(Rectangle::default()).corner_radius_px;
        Self {
            frame_over: Box::new(frame_over),
            corner_radius_px,
            ..self
        }
    }

    #[must_use]
    pub fn dismissed_by(self, dismissal: Message) -> Self {
        Self {
            outside_press: OutsidePress::Dismisses {
                dismissal,
                reachable_below: Vec::new(),
            },
            ..self
        }
    }

    #[must_use]
    pub fn outside_press(self, outside_press: OutsidePress<Message>) -> Self {
        Self {
            outside_press,
            ..self
        }
    }

    #[must_use]
    pub fn leading_alignment(self, leading_alignment_px: f32) -> Self {
        Self {
            leading_alignment_px,
            ..self
        }
    }

    #[must_use]
    pub fn identified_by<OtherIdentity>(
        self,
        shown: Option<OtherIdentity>,
    ) -> AnchoredPopup<'a, Message, OtherIdentity> {
        AnchoredPopup {
            renderings: self.renderings,
            anchor: self.anchor,
            side: self.side,
            gap_px: self.gap_px,
            leading_alignment_px: self.leading_alignment_px,
            morph_duration: self.morph_duration,
            shown,
            outside_press: self.outside_press,
            frame_over: self.frame_over,
            corner_radius_px: self.corner_radius_px,
        }
    }
}

impl<'a, Message: Clone + 'a, ShownIdentity: Clone + PartialEq + 'static>
    From<AnchoredPopup<'a, Message, ShownIdentity>> for Element<'a, Message>
{
    fn from(popup: AnchoredPopup<'a, Message, ShownIdentity>) -> Self {
        Element::new(popup)
    }
}

impl<'a, Message, ShownIdentity: 'static> AnchoredPopup<'a, Message, ShownIdentity> {
    fn anchor_edge_x(&self) -> f32 {
        self.anchor.x + self.anchor.width
    }

    fn dock_line(&self) -> f32 {
        match self.side {
            Edge::Bottom => self.anchor.y + self.anchor.height,
            Edge::Top => self.anchor.y,
            Edge::Right => self.anchor_edge_x(),
            Edge::Left => self.anchor.x,
        }
    }

    fn child_elements(&self) -> Vec<&Element<'a, Message>> {
        self.renderings
            .iter()
            .map(|rendering| &rendering.element)
            .collect()
    }

    fn resting(&self, size: Size, surface: Size) -> Rectangle {
        let centered_on_anchor = |anchor_center: f32, extent: f32, room: f32| {
            (anchor_center - extent / 2.0)
                .min(room - extent - self.gap_px)
                .max(self.gap_px)
        };
        let horizontal = |extent: f32, room: f32| {
            if self.child_framing() == ChildFraming::LeadingChild {
                (self.anchor.center_x() - self.leading_alignment_px)
                    .min(room - extent)
                    .max(0.0)
            } else {
                centered_on_anchor(self.anchor.center_x(), extent, room)
            }
        };
        let dock_line = self.dock_line();
        let top_left = match self.side {
            Edge::Bottom => Point::new(
                horizontal(size.width, surface.width),
                dock_line + self.gap_px,
            ),
            Edge::Top => Point::new(
                horizontal(size.width, surface.width),
                dock_line - self.gap_px - size.height,
            ),
            Edge::Right => Point::new(
                dock_line + self.gap_px,
                centered_on_anchor(self.anchor.center_y(), size.height, surface.height),
            ),
            Edge::Left => Point::new(
                dock_line - self.gap_px - size.width,
                centered_on_anchor(self.anchor.center_y(), size.height, surface.height),
            ),
        };
        Rectangle::new(top_left, size)
    }

    fn views<'t>(
        &'t self,
        tree: &'t Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = ChildView<'t, 'a, Message>> {
        self.renderings
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((rendering, state), layout)| ChildView {
                widget: &rendering.element,
                state,
                layout,
            })
    }

    fn views_mut<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = ChildViewMut<'t, 'a, Message>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((rendering, state), layout)| ChildViewMut {
                widget: &mut rendering.element,
                state,
                layout,
            })
    }

    fn cascade_limits(
        &self,
        orientation: CascadeOrientation,
        primary: Size,
        surface: Size,
    ) -> Limits {
        let resting = self.resting(primary, surface);
        let room_beside = match orientation.direction {
            CascadeDirection::Rightward => surface.width - (resting.x + resting.width),
            CascadeDirection::Leftward => resting.x,
        } - self.gap_px;
        let room_along_growth = match orientation.growth {
            PopupGrowth::Downward => surface.height - resting.y,
            PopupGrowth::Upward => resting.y + resting.height,
        } - self.gap_px;
        Limits::new(
            Size::ZERO,
            Size::new(
                primary.width + room_beside.max(0.0),
                room_along_growth.max(0.0),
            ),
        )
    }

    fn content_origin(&self, popup: Rectangle, framed_height: f32) -> Point {
        match self.side {
            Edge::Bottom | Edge::Right | Edge::Left => popup.position(),
            Edge::Top => Point::new(popup.x, popup.y + popup.height - framed_height),
        }
    }

    fn child_framing(&self) -> ChildFraming {
        if self.renderings.iter().any(|rendering| {
            rendering.arrival == PopupArrival::Incoming
                && matches!(rendering.framing, PopupFraming::LeadingSurface(_))
        }) {
            ChildFraming::LeadingChild
        } else {
            ChildFraming::WholePopup
        }
    }

    fn frame_motion(&self, state: &MorphState<ShownIdentity>) -> MotionState {
        let shows_incoming = self
            .renderings
            .iter()
            .any(|rendering| rendering.arrival == PopupArrival::Incoming);
        match state.morph.motion_state() {
            MotionState::Settled if shows_incoming => MotionState::Settled,
            MotionState::Settled | MotionState::Moving => MotionState::Moving,
        }
    }

    fn popup_frame(&self, state: &MorphState<ShownIdentity>, layout: Layout<'_>) -> PopupFrame {
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        PopupFrame {
            surface_rows: state.connector.translated(origin).surface_rows(
                state.morph.current() + origin,
                Radius::from(self.corner_radius_px),
            ),
            motion: self.frame_motion(state),
        }
    }

    fn pointer_placement(&self, tree: &Tree, layout: Layout<'_>, point: Point) -> PointerPlacement {
        let state: &MorphState<ShownIdentity> = tree.state.downcast_ref();
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        let over_popup = (state.morph.current() + origin).contains(point)
            || self
                .renderings
                .iter()
                .zip(layout.children())
                .filter(|(rendering, _)| {
                    rendering.arrival == PopupArrival::Incoming
                        && matches!(rendering.framing, PopupFraming::LeadingSurface(_))
                })
                .flat_map(|(_, cascade)| cascade.children().skip(1))
                .any(|satellite| {
                    let node = satellite.bounds();
                    satellite.children().any(|surface| {
                        let surface_bounds = surface.bounds();
                        Rectangle::new(
                            Point::new(node.x, surface_bounds.y),
                            Size::new(node.width, surface_bounds.height),
                        )
                        .contains(point)
                    })
                });
        if over_popup {
            PointerPlacement::OverPopup
        } else {
            PointerPlacement::OutsidePopup
        }
    }

    fn live_view<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> Option<ChildViewMut<'t, 'a, Message>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .find(|((rendering, _), _)| rendering.arrival == PopupArrival::Incoming)
            .map(|((rendering, state), layout)| ChildViewMut {
                widget: &mut rendering.element,
                state,
                layout,
            })
    }
}

impl<Message: Clone, ShownIdentity: Clone + PartialEq + 'static>
    Widget<Message, iced::Theme, iced::Renderer> for AnchoredPopup<'_, Message, ShownIdentity>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MorphState<ShownIdentity>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(MorphState::<ShownIdentity> {
            shown: None,
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
        let switched = tree.state.downcast_ref::<MorphState<ShownIdentity>>().shown != self.shown;
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
        let rendering_limits = Limits::new(Size::ZERO, surface);
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
        if state.shown != self.shown {
            state.shown.clone_from(&self.shown);
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
        let box_bounds = state.morph.current() + origin;
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
                popup_bounds: state.morph.current() + origin,
                bounds: layout.bounds(),
                takes_input: frame.motion == MotionState::Settled,
                captures_outside_presses: state.morph.motion_state() == MotionState::Settled
                    && matches!(self.outside_press, OutsidePress::Dismisses { .. }),
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
