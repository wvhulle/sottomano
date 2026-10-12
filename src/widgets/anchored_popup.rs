use std::iter::once;

use iced::{
    Animation, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};

use crate::{
    config::Config,
    desktop_shell::{Message, monitor_surfaces::Position},
    state::ShellSettings,
    widgets::{
        child_view::{ChildView, ChildViewMut},
        popup_connector::{notch, notch_blur_rows, notch_region},
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
    settings: &ShellSettings,
) -> Element<'a, Message> {
    let background = config
        .theme
        .palette
        .surface
        .scale_alpha(settings.overlays.opacity);
    let outline = settings.outline.border(config);
    Element::new(AnchoredPopup {
        renderings,
        notch: notch(position, background, outline),
        anchor,
        edge_inset: config.panel_bar.height_px + config.panel_bar.popup_gap_px,
        bar_thickness: config.panel_bar.height_px,
        popup_gap: config.panel_bar.popup_gap_px,
        position,
        seq,
        morph: config.tooltips.popup_morph,
        background,
        radius: settings.overlays.corner_radius.top_left,
        shadow: settings.shadow.drop_shadow(),
        outline,
    })
}

struct AnchoredPopup<'a> {
    renderings: Vec<Rendering<'a>>,
    notch: Element<'a, Message>,
    anchor: Rectangle,
    edge_inset: f32,
    bar_thickness: f32,
    popup_gap: f32,
    position: Position,
    seq: u64,
    morph: Duration,
    background: Color,
    radius: f32,
    shadow: Shadow,
    outline: Border,
}

struct MorphState {
    seq: u64,
    morph: Animation<bool>,
    now: Instant,
    progress: f32,
    from: Rectangle,
    current: Rectangle,
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
        once(&self.notch)
            .chain(self.renderings.iter().map(|rendering| &rendering.element))
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
            .zip(&tree.children[1..])
            .zip(layout.children().skip(1))
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
            .zip(&mut tree.children[1..])
            .zip(layout.children().skip(1))
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

    fn notch_join(&self, box_bounds: Rectangle, notch_bounds: Rectangle) -> Rectangle {
        let reach = 2.0 * self.outline.width;
        match self.position {
            Position::Top => Rectangle {
                x: notch_bounds.x,
                y: box_bounds.y,
                width: notch_bounds.width,
                height: reach,
            },
            Position::Bottom => Rectangle {
                x: notch_bounds.x,
                y: box_bounds.y + box_bounds.height - reach,
                width: notch_bounds.width,
                height: reach,
            },
            Position::Left => Rectangle {
                x: box_bounds.x,
                y: notch_bounds.y,
                width: reach,
                height: notch_bounds.height,
            },
        }
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
        let notch_rows = if settled {
            let notch_bounds = layout.children().next().unwrap().bounds();
            notch_blur_rows(notch_bounds, self.position)
        } else {
            Vec::new()
        };
        PopupFrame {
            bounds: state.current + Vector::new(layout.bounds().x, layout.bounds().y),
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
            .zip(&mut tree.children[1..])
            .zip(layout.children().skip(1))
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
            now: Instant::now(),
            progress: 1.0,
            from: Rectangle::default(),
            current: Rectangle::default(),
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
                .zip(&mut tree.children[1..])
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
                .duration(self.morph)
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
                shadow: self.shadow,
                ..Default::default()
            },
            self.background,
        );
        let settled = self.is_settled(state);
        let notch_layout = layout.children().next().unwrap();
        if settled {
            self.notch.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                notch_layout,
                cursor,
                viewport,
            );
        }
        let notch_join = settled.then(|| self.notch_join(box_bounds, notch_layout.bounds()));
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
            blurred: true,
            corner_radius: self.radius,
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
        state.now = *now;
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
