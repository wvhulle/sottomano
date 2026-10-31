use std::iter::once;

use iced::{
    Element, Event, Length, Point, Rectangle, Shadow, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::Cursor,
};
use sottomano_config::{Config, screen_edges::Position};

use crate::{
    hover_tooltips::tooltip_transition::ShownTarget,
    widgets::{
        child_view::{ChildView, ChildViewMut},
        panel_frame::LayerFrameStyle,
        popup_connector::{ConnectorTriangle, FramedSurface},
        rectangle_morph::interpolated,
        region_reporter::RegionRoles,
        shell_layer_appearance::LayerAppearance,
    },
};

struct PopupFrame {
    bounds: Rectangle,
    notch_rows: Vec<Rectangle>,
    settled: bool,
}

impl PopupFrame {
    fn rects(&self) -> impl Iterator<Item = Rectangle> {
        once(self.bounds).chain(self.notch_rows.iter().copied())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupFraming {
    WholeContent,
    FirstChild,
}

impl PopupFraming {
    fn framed_size(self, node: &Node) -> Size {
        match self {
            PopupFraming::WholeContent => node.size(),
            PopupFraming::FirstChild => node
                .children()
                .first()
                .map_or_else(|| node.size(), Node::size),
        }
    }
}

pub struct Rendering<'a, Message> {
    element: Element<'a, Message>,
    incoming: bool,
    framing: PopupFraming,
}

impl<'a, Message> Rendering<'a, Message> {
    #[must_use]
    pub fn incoming(element: Element<'a, Message>) -> Self {
        Self {
            element,
            incoming: true,
            framing: PopupFraming::WholeContent,
        }
    }

    #[must_use]
    pub fn outgoing(element: Element<'a, Message>) -> Self {
        Self {
            element,
            incoming: false,
            framing: PopupFraming::WholeContent,
        }
    }

    #[must_use]
    pub fn framing(self, framing: PopupFraming) -> Self {
        Self { framing, ..self }
    }
}

#[must_use]
pub fn anchored_popup<'a, Message: 'a>(
    renderings: Vec<Rendering<'a, Message>>,
    anchor: Rectangle,
    position: Position,
    morph: PopupMorph,
    config: &Config,
    overlay_appearance: LayerAppearance,
) -> Element<'a, Message> {
    let overlay_frame = LayerFrameStyle::new(&overlay_appearance, config);
    Element::new(AnchoredPopup {
        renderings,
        anchor,
        edge_inset: config.layout.panel.height + config.layout.overlay.panel_gap,
        bar_thickness: config.layout.panel.height,
        popup_gap: config.layout.overlay.panel_gap,
        leading_icon_center: config.layout.overlay.content_inset + config.layout.panel.height / 2.0,
        position,
        morph,
        frame: overlay_frame,
        radius: overlay_appearance.corner_radius_px,
        blurred: overlay_appearance.blurred,
        shadow: overlay_frame.lifted_drop_shadow(),
    })
}

struct AnchoredPopup<'a, Message> {
    renderings: Vec<Rendering<'a, Message>>,
    anchor: Rectangle,
    edge_inset: f32,
    bar_thickness: f32,
    popup_gap: f32,
    leading_icon_center: f32,
    position: Position,
    morph: PopupMorph,
    frame: LayerFrameStyle,
    radius: f32,
    blurred: bool,
    shadow: Shadow,
}

#[derive(Debug, Clone)]
pub struct PopupMorph {
    pub progress: f32,
    pub shown: ShownTarget,
}

struct MorphState {
    shown: ShownTarget,
    origin: Rectangle,
    current: Rectangle,
    connector: ConnectorTriangle,
}

impl<'a, Message> AnchoredPopup<'a, Message> {
    fn anchor_edge_x(&self) -> f32 {
        self.anchor.x + self.anchor.width
    }

    fn bar_edge(&self, surface: Size) -> f32 {
        match self.position {
            Position::Top => self.bar_thickness,
            Position::Bottom => surface.height - self.bar_thickness,
            Position::Left => self.anchor_edge_x(),
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
                .min(room - extent - self.popup_gap)
                .max(self.popup_gap)
        };
        let horizontal = |extent: f32, room: f32| {
            if self.frames_first_child() {
                (self.anchor.center_x() - self.leading_icon_center)
                    .min(room - extent)
                    .max(0.0)
            } else {
                centered_on_anchor(self.anchor.center_x(), extent, room)
            }
        };
        let top_left = match self.position {
            Position::Top => Point::new(horizontal(size.width, surface.width), self.edge_inset),
            Position::Bottom => Point::new(
                horizontal(size.width, surface.width),
                surface.height - self.edge_inset - size.height,
            ),
            Position::Left => Point::new(
                self.anchor_edge_x() + self.popup_gap,
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

    fn content_origin(&self, popup: Rectangle, framed_height: f32) -> Point {
        match self.position {
            Position::Top | Position::Left => popup.position(),
            Position::Bottom => Point::new(popup.x, popup.y + popup.height - framed_height),
        }
    }

    fn frames_first_child(&self) -> bool {
        self.renderings
            .iter()
            .any(|rendering| rendering.incoming && rendering.framing == PopupFraming::FirstChild)
    }

    fn is_live(&self) -> bool {
        self.renderings.iter().any(|rendering| rendering.incoming)
    }

    fn is_settled(&self) -> bool {
        self.morph.progress >= 1.0 && self.is_live()
    }

    fn frame(&self, state: &MorphState, layout: Layout<'_>) -> PopupFrame {
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        PopupFrame {
            bounds: state.current + origin,
            notch_rows: state.connector.translated(origin).blur_rows(),
            settled: self.is_settled(),
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
            .find(|((rendering, _), _)| rendering.incoming)
            .map(|((rendering, state), layout)| ChildViewMut {
                widget: &mut rendering.element,
                state,
                layout,
            })
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for AnchoredPopup<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MorphState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(MorphState {
            shown: ShownTarget::Nothing,
            origin: Rectangle::default(),
            current: Rectangle::default(),
            connector: ConnectorTriangle::between(
                self.anchor,
                0.0,
                Rectangle::default(),
                self.position,
                self.radius,
            ),
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.child_elements().into_iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let switched = tree.state.downcast_ref::<MorphState>().shown != self.morph.shown;
        tree.diff_children(&self.child_elements());
        if switched {
            self.renderings
                .iter()
                .zip(&mut tree.children)
                .filter(|(rendering, _)| rendering.incoming)
                .for_each(|(rendering, child)| *child = Tree::new(&rendering.element));
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let surface = limits.max();
        let rendering_limits = Limits::new(Size::ZERO, surface);
        let nodes: Vec<Node> = self
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

        let arriving = self
            .renderings
            .iter()
            .zip(&nodes)
            .find(|(rendering, _)| rendering.incoming)
            .map(|(rendering, node)| rendering.framing.framed_size(node));
        let at_anchor = self.resting(Size::ZERO, surface);

        let state: &mut MorphState = tree.state.downcast_mut();
        if state.shown != self.morph.shown {
            state.shown = self.morph.shown.clone();
            state.origin = if state.current.width > 0.0 {
                state.current
            } else {
                at_anchor
            };
        }
        let target = arriving.map_or(at_anchor, |size| self.resting(size, surface));
        state.current = interpolated(state.origin, target, self.morph.progress);
        let popup = state.current;
        state.connector = ConnectorTriangle::between(
            self.anchor,
            self.bar_edge(surface),
            popup,
            self.position,
            self.radius,
        );

        let placed = self
            .renderings
            .iter()
            .zip(nodes)
            .map(|(rendering, node)| {
                let framed_height = rendering.framing.framed_size(&node).height;
                node.move_to(self.content_origin(popup, framed_height))
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
        let state: &MorphState = tree.state.downcast_ref();
        let bounds = layout.bounds();
        let origin = Vector::new(bounds.x, bounds.y);
        let box_bounds = state.current + origin;
        if box_bounds.width <= 0.0 || box_bounds.height <= 0.0 {
            return;
        }
        state.connector.translated(origin).draw_framed_surface(
            renderer,
            theme,
            FramedSurface {
                frame: self.frame,
                bounds: box_bounds,
                corners: Radius::from(self.radius),
                shadow: self.shadow,
            },
        );
        let content_clip = if self.frames_first_child() && self.is_settled() {
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
        let state: &MorphState = tree.state.downcast_ref();
        let frame = self.frame(state, layout);
        let roles = RegionRoles {
            takes_input: frame.settled,
            blurred: self.blurred,
            corner_radius: Radius::from(self.radius),
        };
        let rects: Vec<Rectangle> = frame.rects().collect();
        roles.report(&rects, layout.bounds(), operation);
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
            .unwrap_or_default()
    }
}
