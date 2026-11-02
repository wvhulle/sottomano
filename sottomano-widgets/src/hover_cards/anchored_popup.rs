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
    desktop_panels::{panel_frame::LayerFrameStyle, region_reporter::RegionRoles},
    group_menus::cascading_layout::{
        cascade_row::{CascadeDirection, CascadeOrientation},
        notched_panel::PanelGrowth,
    },
    hover_cards::{
        hover_card_transition::ShownTarget,
        popup_connector::{ConnectorTriangle, FramedSurface},
    },
    widgets::{
        child_view::{ChildView, ChildViewMut},
        rectangle_morph::interpolated,
        shell_layer_appearance::LayerAppearance,
    },
};

struct PopupFrame {
    surface_rows: Vec<Rectangle>,
    settled: bool,
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
        edge_inset: config.desktop_panels.height + config.overlay_surfaces.satellite_gap,
        panel_thickness: config.desktop_panels.height,
        popup_gap: config.overlay_surfaces.satellite_gap,
        leading_icon_center: config.overlay_surfaces.content_inset
            + config.desktop_panels.height / 2.0,
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
    panel_thickness: f32,
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

    fn panel_edge(&self, surface: Size) -> f32 {
        match self.position {
            Position::Top => self.panel_thickness,
            Position::Bottom => surface.height - self.panel_thickness,
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
            if self.frames_one_child() {
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
        } - self.popup_gap;
        let room_along_growth = match orientation.growth {
            PanelGrowth::Downward => surface.height - resting.y,
            PanelGrowth::Upward => resting.y + resting.height,
        } - self.popup_gap;
        Limits::new(
            Size::ZERO,
            Size::new(
                primary.width + room_beside.max(0.0),
                room_along_growth.max(primary.height),
            ),
        )
    }

    fn content_origin(&self, popup: Rectangle, framed_height: f32) -> Point {
        match self.position {
            Position::Top | Position::Left => popup.position(),
            Position::Bottom => Point::new(popup.x, popup.y + popup.height - framed_height),
        }
    }

    fn frames_one_child(&self) -> bool {
        self.renderings.iter().any(|rendering| {
            rendering.incoming && matches!(rendering.framing, PopupFraming::LeadingSurface(_))
        })
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
            surface_rows: state
                .connector
                .translated(origin)
                .surface_rows(state.current + origin, Radius::from(self.radius)),
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
                self.position.into(),
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
            .find(|(rendering, _)| rendering.incoming)
            .map(|(rendering, node)| rendering.framing.framed_bounds(node).size());
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
            self.panel_edge(surface),
            popup,
            self.position.into(),
            self.radius,
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
        let content_clip = if self.frames_one_child() && self.is_settled() {
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
            corner_radius: Radius::default(),
        };
        roles.report(&frame.surface_rows, layout.bounds(), operation);
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
