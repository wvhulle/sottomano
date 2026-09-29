use iced::{
    Border, Color, Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    widget::canvas::Frame,
    window,
};

use crate::{
    app::Message,
    config::Config,
    wayland_layers::surface::Position,
    widgets::{overlay_shadow::overlay_shadow, popup_connector::notch},
};

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
    progress: f32,
    seq: u64,
    config: &Config,
    on_bounds: impl Fn(Rectangle) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(AnchoredPopup {
        renderings,
        on_bounds: Box::new(on_bounds),
        anchor,
        edge_inset: config.chrome.outer_height_px + config.chrome.popup_gap_px,
        bar_thickness: config.chrome.outer_height_px,
        popup_gap: config.chrome.popup_gap_px,
        position,
        progress: progress.clamp(0.0, 1.0),
        seq,
        background: config.palette.surface,
        radius: f32::from(config.chrome.popup_radius_px),
    })
}

struct AnchoredPopup<'a> {
    renderings: Vec<Rendering<'a>>,
    on_bounds: Box<dyn Fn(Rectangle) -> Message + 'a>,
    anchor: Rectangle,
    edge_inset: f32,
    bar_thickness: f32,
    popup_gap: f32,
    position: Position,
    progress: f32,
    seq: u64,
    background: Color,
    radius: f32,
}

#[derive(Default)]
struct MorphState {
    seq: u64,
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
    fn pill_edge_x(&self) -> f32 {
        self.anchor.x + self.anchor.width
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
                self.pill_edge_x() + self.popup_gap,
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
            .zip(&tree.children)
            .zip(layout.children())
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
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((rendering, state), layout)| RenderingViewMut {
                widget: &mut rendering.element,
                state,
                layout,
            })
    }

    fn is_live(&self) -> bool {
        self.renderings.iter().any(|rendering| rendering.incoming)
    }

    fn is_settled(&self) -> bool {
        self.progress >= 1.0 && self.is_live()
    }

    fn live_view<'t>(
        &'t mut self,
        tree: &'t mut Tree,
        layout: Layout<'t>,
    ) -> Option<RenderingViewMut<'t, 'a>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
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
        tree::State::new(MorphState::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.renderings
            .iter()
            .map(|rendering| Tree::new(&rendering.element))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children_custom(
            &self.renderings,
            |state, rendering| state.diff(&rendering.element),
            |rendering| Tree::new(&rendering.element),
        );
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
        }
        let target = arriving.map_or(at_anchor, |size| self.resting(size, surface));
        state.current = lerp(state.from, target, self.progress);
        let origin = state.current.position();

        Node::with_children(
            surface,
            nodes.into_iter().map(|node| node.move_to(origin)).collect(),
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
        let bar_edge = match self.position {
            Position::Top => bounds.y + self.bar_thickness,
            Position::Bottom => bounds.y + bounds.height - self.bar_thickness,
            Position::Left => bounds.x + self.pill_edge_x(),
        };
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
        if self.is_settled() {
            let notch = notch(
                self.anchor + origin,
                bar_edge,
                box_bounds,
                self.position,
                self.radius,
            );
            let fill = self.background;
            renderer.with_translation(Vector::new(notch.region.x, notch.region.y), |renderer| {
                let mut frame = Frame::new(renderer, notch.region.size());
                frame.fill(&notch.path, fill);
                renderer.draw_geometry(frame.into_geometry());
            });
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
        if !matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            return;
        }
        let live = self.is_live();
        let state: &mut MorphState = tree.state.downcast_mut();
        let interactive = if live {
            state.current + Vector::new(layout.bounds().x, layout.bounds().y)
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
