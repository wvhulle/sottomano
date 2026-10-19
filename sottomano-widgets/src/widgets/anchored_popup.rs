use std::{iter::once, mem};

use iced::{
    Animation, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    border::Radius,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use sottomano_config::{Config, screen_edges::Position};
use sottomano_state::ShellSettings;

use crate::{
    shell_message::Message,
    widgets::{
        child_view::{ChildView, ChildViewMut},
        palette_colors::PaletteColors,
        panel_frame::LayerFrameStyle,
        popup_connector::ConnectorTriangle,
        region_reporter::RegionRoles,
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

pub struct Rendering<'a> {
    element: Element<'a, Message>,
    incoming: bool,
}

impl<'a> Rendering<'a> {
    #[must_use]
    pub fn incoming(element: Element<'a, Message>) -> Self {
        Self {
            element,
            incoming: true,
        }
    }

    #[must_use]
    pub fn outgoing(element: Element<'a, Message>) -> Self {
        Self {
            element,
            incoming: false,
        }
    }
}

#[must_use]
pub fn anchored_popup<'a>(
    renderings: Vec<Rendering<'a>>,
    anchor: Rectangle,
    position: Position,
    seq: u64,
    config: &Config,
    settings: &ShellSettings,
) -> Element<'a, Message> {
    let background = PaletteColors::from(&config.theme.palette)
        .surface
        .scale_alpha(settings.overlays.opacity);
    let overlays = &settings.overlays;
    let overlay_frame = LayerFrameStyle::new(overlays, config);
    Element::new(AnchoredPopup {
        renderings,
        anchor,
        edge_inset: config.layout.panel.height + config.layout.overlay.panel_gap,
        bar_thickness: config.layout.panel.height,
        popup_gap: config.layout.overlay.panel_gap,
        position,
        seq,
        morph: config.tooltips.popup_morph,
        background,
        radius: overlays.corner_radius_px,
        blurred: overlays.blurred,
        shadow: overlay_frame.drop_shadow(),
        outline: overlay_frame.outline_border(),
    })
}

struct AnchoredPopup<'a> {
    renderings: Vec<Rendering<'a>>,
    anchor: Rectangle,
    edge_inset: f32,
    bar_thickness: f32,
    popup_gap: f32,
    position: Position,
    seq: u64,
    morph: Duration,
    background: Color,
    radius: f32,
    blurred: bool,
    shadow: Shadow,
    outline: Border,
}

struct MorphState {
    seq: u64,
    morph: Animation<bool>,
    morph_pending: bool,
    now: Option<Instant>,
    progress: f32,
    from: Rectangle,
    current: Rectangle,
    connector: ConnectorTriangle,
}

fn around_hole(outer: Rectangle, hole: Rectangle) -> Vec<Rectangle> {
    let outer_right = outer.x + outer.width;
    let outer_bottom = outer.y + outer.height;
    let hole_right = hole.x + hole.width;
    let hole_bottom = hole.y + hole.height;
    [
        Rectangle {
            x: outer.x,
            y: outer.y,
            width: outer.width,
            height: hole.y - outer.y,
        },
        Rectangle {
            x: outer.x,
            y: hole_bottom,
            width: outer.width,
            height: outer_bottom - hole_bottom,
        },
        Rectangle {
            x: outer.x,
            y: hole.y,
            width: hole.x - outer.x,
            height: hole.height,
        },
        Rectangle {
            x: hole_right,
            y: hole.y,
            width: outer_right - hole_right,
            height: hole.height,
        },
    ]
    .into_iter()
    .filter(|part| part.width > 0.0 && part.height > 0.0)
    .collect()
}

fn lerp(from: Rectangle, to: Rectangle, fraction: f32) -> Rectangle {
    let mix = |start: f32, end: f32| start + (end - start) * fraction;
    Rectangle {
        x: mix(from.x, to.x),
        y: mix(from.y, to.y),
        width: mix(from.width, to.width),
        height: mix(from.height, to.height),
    }
}

impl<'a> AnchoredPopup<'a> {
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
        let top_left = match self.position {
            Position::Top => Point::new(
                centered_on_anchor(self.anchor.center_x(), size.width, surface.width),
                self.edge_inset,
            ),
            Position::Bottom => Point::new(
                centered_on_anchor(self.anchor.center_x(), size.width, surface.width),
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

    fn is_live(&self) -> bool {
        self.renderings.iter().any(|rendering| rendering.incoming)
    }

    fn is_settled(&self, state: &MorphState) -> bool {
        state.progress >= 1.0 && self.is_live()
    }

    fn draw_outline(
        &self,
        renderer: &mut iced::Renderer,
        box_bounds: Rectangle,
        notch_join: Option<Rectangle>,
    ) {
        if self.outline.width <= 0.0 {
            return;
        }
        let quad = renderer::Quad {
            bounds: box_bounds,
            border: Border {
                radius: self.radius.into(),
                ..self.outline
            },
            ..Default::default()
        };
        let clips =
            notch_join.map_or_else(|| vec![box_bounds], |join| around_hole(box_bounds, join));
        for clip in clips {
            renderer.with_layer(clip, |renderer| {
                renderer.fill_quad(quad, Color::TRANSPARENT);
            });
        }
    }

    fn frame(&self, state: &MorphState, layout: Layout<'_>) -> PopupFrame {
        let settled = self.is_settled(state);
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        let notch_rows = if settled {
            state.connector.translated(origin).blur_rows()
        } else {
            Vec::new()
        };
        PopupFrame {
            bounds: state.current + origin,
            notch_rows,
            settled,
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

impl Widget<Message, iced::Theme, iced::Renderer> for AnchoredPopup<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MorphState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(MorphState {
            seq: 0,
            morph: Animation::new(true),
            morph_pending: false,
            now: None,
            progress: 1.0,
            from: Rectangle::default(),
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
        let switched = tree.state.downcast_ref::<MorphState>().seq != self.seq;
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
            .map(|(_, node)| node.size());
        let at_anchor = self.resting(Size::ZERO, surface);

        let state: &mut MorphState = tree.state.downcast_mut();
        if state.seq != self.seq {
            state.seq = self.seq;
            state.from = if state.current.width > 0.0 {
                state.current
            } else {
                at_anchor
            };
            state.morph_pending = true;
        }
        state.progress = if state.morph_pending {
            0.0
        } else {
            state
                .now
                .map_or(1.0, |now| state.morph.interpolate(0.0, 1.0, now))
        };
        let target = arriving.map_or(at_anchor, |size| self.resting(size, surface));
        state.current = lerp(state.from, target, state.progress);
        let popup = state.current;
        state.connector = ConnectorTriangle::between(
            self.anchor,
            self.bar_edge(surface),
            popup,
            self.position,
            self.radius,
        );

        Node::with_children(
            surface,
            nodes
                .into_iter()
                .map(|node| node.move_to(popup.position()))
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
        let state: &MorphState = tree.state.downcast_ref();
        let bounds = layout.bounds();
        let origin = Vector::new(bounds.x, bounds.y);
        let box_bounds = state.current + origin;
        if box_bounds.width <= 0.0 || box_bounds.height <= 0.0 {
            return;
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds: box_bounds,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: self.radius.into(),
                },
                shadow: self.shadow,
                ..Default::default()
            },
            self.background,
        );
        let notch_join = self.is_settled(state).then(|| {
            let connector = state.connector.translated(origin);
            connector.draw(renderer, self.background, self.outline);
            connector.base_span(2.0 * self.outline.width)
        });
        self.draw_outline(renderer, box_bounds, notch_join);
        renderer.with_layer(box_bounds, |renderer| {
            self.views(tree, layout).for_each(|rendering| {
                rendering.widget.as_widget().draw(
                    rendering.state,
                    renderer,
                    theme,
                    style,
                    rendering.layout,
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
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let state: &mut MorphState = tree.state.downcast_mut();
        state.now = Some(*now);
        if mem::take(&mut state.morph_pending) {
            state.morph = Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(self.morph)
                .go(true, *now);
        }
        if state.progress < 1.0 {
            shell.invalidate_layout();
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
