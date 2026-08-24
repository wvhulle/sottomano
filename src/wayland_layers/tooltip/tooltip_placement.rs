use iced::{
    Border, Color, Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

use crate::{
    app::Message, config::Config, wayland_layers::surface::Position,
    widgets::overlay_shadow::overlay_shadow,
};

const BAR_GAP: f32 = 6.0;

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

pub fn morphing_tooltip<'a>(
    renderings: Vec<Rendering<'a>>,
    anchor: Rectangle,
    position: Position,
    progress: f32,
    seq: u64,
    config: &Config,
) -> Element<'a, Message> {
    Element::new(MorphingTooltip {
        renderings,
        anchor,
        bar_extent: config.chrome.outer_height_px,
        position,
        progress: progress.clamp(0.0, 1.0),
        seq,
        background: Color {
            a: 1.0,
            ..config.palette.bar_background
        },
        radius: f32::from(config.chrome.popup_radius_px),
    })
}

struct MorphingTooltip<'a> {
    renderings: Vec<Rendering<'a>>,
    anchor: Rectangle,
    bar_extent: f32,
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
}

fn resting(
    anchor: Rectangle,
    size: Size,
    surface: Size,
    position: Position,
    bar_extent: f32,
) -> Rectangle {
    let x = anchor.x.clamp(0.0, (surface.width - size.width).max(0.0));
    let y = match position {
        Position::Top => bar_extent + BAR_GAP,
        Position::Bottom => surface.height - bar_extent - size.height - BAR_GAP,
    };
    Rectangle {
        x,
        y,
        width: size.width,
        height: size.height,
    }
}

fn pinned_to_anchor(
    anchor: Rectangle,
    surface: Size,
    position: Position,
    bar_extent: f32,
) -> Rectangle {
    let y = match position {
        Position::Top => bar_extent + BAR_GAP,
        Position::Bottom => surface.height - bar_extent - BAR_GAP,
    };
    Rectangle {
        x: anchor.center_x(),
        y,
        width: 0.0,
        height: 0.0,
    }
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

impl<'a> MorphingTooltip<'a> {
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
}

impl Widget<Message, iced::Theme, iced::Renderer> for MorphingTooltip<'_> {
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
        let at_anchor = pinned_to_anchor(self.anchor, surface, self.position, self.bar_extent);

        let state: &mut MorphState = tree.state.downcast_mut();
        if state.seq != self.seq {
            state.seq = self.seq;
            state.from = if state.current.width > 0.0 {
                state.current
            } else {
                at_anchor
            };
        }
        let target = arriving.map_or(at_anchor, |size| {
            resting(self.anchor, size, surface, self.position, self.bar_extent)
        });
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
        let box_bounds = state.current;
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
        self.views_mut(tree, layout).for_each(|rendering| {
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
        });
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
