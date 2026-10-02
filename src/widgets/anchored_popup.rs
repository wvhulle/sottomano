use std::iter::once;

use iced::{
    Animation, Border, Color, Element, Event, Length, Point, Rectangle, Size, Vector,
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

use crate::{
    desktop_shell::Message,
    config::Config,
    desktop_shell::monitor_surfaces::Position,
    widgets::{
        overlay_shadow::overlay_shadow,
        popup_connector::{notch, notch_region},
    },
};

const MORPH: Duration = Duration::from_millis(160);

pub struct Rendering<'a> {
    element: Element<'a, Message>,
    incoming: bool,
}

impl<'a> Rendering<'a> {
    pub fn incoming(element: Element<'a, Message>) -> Self {
        Self {
            element,
            incoming: true,
        }
    }

    pub fn outgoing(element: Element<'a, Message>) -> Self {
        Self {
            element,
            incoming: false,
        }
    }
}

pub fn anchored_popup<'a>(
    renderings: Vec<Rendering<'a>>,
    anchor: Rectangle,
    position: Position,
    seq: u64,
    config: &Config,
    corner_radius: Radius,
    on_bounds: impl Fn(Rectangle) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(AnchoredPopup {
        renderings,
        notch: notch(position, config.palette.surface),
        on_bounds: Box::new(on_bounds),
        anchor,
        edge_inset: config.bar.height_px + config.bar.popup_gap_px,
        bar_thickness: config.bar.height_px,
        popup_gap: config.bar.popup_gap_px,
        position,
        seq,
        background: config.palette.surface,
        radius: corner_radius.top_left,
    })
}

struct AnchoredPopup<'a> {
    renderings: Vec<Rendering<'a>>,
    notch: Element<'a, Message>,
    on_bounds: Box<dyn Fn(Rectangle) -> Message + 'a>,
    anchor: Rectangle,
    edge_inset: f32,
    bar_thickness: f32,
    popup_gap: f32,
    position: Position,
    seq: u64,
    background: Color,
    radius: f32,
}

struct MorphState {
    seq: u64,
    morph: Animation<bool>,
    now: Instant,
    progress: f32,
    from: Rectangle,
    current: Rectangle,
    published: Option<Rectangle>,
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

struct RenderingView<'t, 'a> {
    widget: &'t Element<'a, Message>,
    state: &'t Tree,
    layout: Layout<'t>,
}

struct RenderingViewMut<'t, 'a> {
    widget: &'t mut Element<'a, Message>,
    state: &'t mut Tree,
    layout: Layout<'t>,
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
        once(&self.notch)
            .chain(self.renderings.iter().map(|rendering| &rendering.element))
            .collect()
    }

    fn resting(&self, size: Size, surface: Size) -> Rectangle {
        let centered_on_anchor = |anchor_center: f32, extent: f32, room: f32| {
            (anchor_center - extent / 2.0).clamp(0.0, (room - extent).max(0.0))
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
    ) -> impl DoubleEndedIterator<Item = RenderingView<'t, 'a>> {
        self.renderings
            .iter()
            .zip(&tree.children[1..])
            .zip(layout.children().skip(1))
            .map(|((rendering, state), layout)| RenderingView {
                widget: &rendering.element,
                state,
                layout,
            })
    }

    fn views_mut<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> impl DoubleEndedIterator<Item = RenderingViewMut<'t, 'a>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children[1..])
            .zip(layout.children().skip(1))
            .map(|((rendering, state), layout)| RenderingViewMut {
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

    fn live_view<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> Option<RenderingViewMut<'t, 'a>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children[1..])
            .zip(layout.children().skip(1))
            .find(|((rendering, _), _)| rendering.incoming)
            .map(|((rendering, state), layout)| RenderingViewMut {
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
            now: Instant::now(),
            progress: 1.0,
            from: Rectangle::default(),
            current: Rectangle::default(),
            published: None,
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.child_elements().into_iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.child_elements());
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
            .zip(&mut tree.children[1..])
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
            state.now = Instant::now();
            state.morph = Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(MORPH)
                .go(true, state.now);
        }
        state.progress = state.morph.interpolate(0.0, 1.0, state.now);
        let target = arriving.map_or(at_anchor, |size| self.resting(size, surface));
        state.current = lerp(state.from, target, state.progress);
        let popup = state.current;

        let region = notch_region(
            self.anchor,
            self.bar_edge(surface),
            popup,
            self.position,
            self.radius,
        );
        let notch_node = self
            .notch
            .as_widget_mut()
            .layout(
                &mut tree.children[0],
                renderer,
                &Limits::new(Size::ZERO, region.size()),
            )
            .move_to(region.position());

        Node::with_children(
            surface,
            once(notch_node)
                .chain(nodes.into_iter().map(|node| node.move_to(popup.position())))
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
                shadow: overlay_shadow(1.0),
                ..Default::default()
            },
            self.background,
        );
        if self.is_settled(state) {
            self.notch.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                cursor,
                viewport,
            );
        }
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
        state.now = *now;
        if state.progress < 1.0 {
            shell.invalidate_layout();
            shell.request_redraw();
        }
        let interactive = if self.is_settled(state) {
            state.current + Vector::new(layout.bounds().x, layout.bounds().y)
        } else if self.is_live() {
            return;
        } else {
            Rectangle::default()
        };
        if state.published != Some(interactive) {
            state.published = Some(interactive);
            shell.publish((self.on_bounds)(interactive));
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
