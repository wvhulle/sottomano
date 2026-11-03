use std::slice::from_ref;

use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    window,
};

use crate::{
    desktop_panels::panel_frame::LayerFrameStyle,
    widgets::{
        edge_alpha_fade::{FadedEdge, draw_edge_alpha_fade},
        reveal::clip_cursor,
        shared_metrics::SCROLL_LINE_STEP_PX,
    },
};

const OVERFLOW_TOLERANCE_PX: f32 = 0.5;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowListMetrics {
    viewport_width: f32,
    content_width: f32,
}

impl WindowListMetrics {
    #[must_use]
    pub fn max_scroll(&self) -> f32 {
        (self.content_width - self.viewport_width).max(0.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WindowListEdgeFade {
    pub frame: LayerFrameStyle,
    pub width_px: f32,
}

impl WindowListEdgeFade {
    fn band_width(self, viewport_width: f32) -> f32 {
        self.width_px.min(viewport_width / 2.0)
    }

    fn draw_at_left(self, renderer: &mut iced::Renderer, theme: &iced::Theme, bounds: Rectangle) {
        draw_edge_alpha_fade(
            renderer,
            Rectangle {
                width: self.band_width(bounds.width),
                ..bounds
            },
            FadedEdge::Left,
            self.frame.fill_color(theme),
        );
    }

    fn draw_at_right(self, renderer: &mut iced::Renderer, theme: &iced::Theme, bounds: Rectangle) {
        let band_width = self.band_width(bounds.width);
        draw_edge_alpha_fade(
            renderer,
            Rectangle {
                x: bounds.x + bounds.width - band_width,
                width: band_width,
                ..bounds
            },
            FadedEdge::Right,
            self.frame.fill_color(theme),
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct FocusedWindowSpan {
    left: f32,
    right: f32,
}

impl FocusedWindowSpan {
    fn offset_clear_of_fades(self, offset: f32, viewport_width: f32, fade_width: f32) -> f32 {
        if self.left < offset + fade_width {
            self.left - fade_width
        } else if self.right > offset + viewport_width - fade_width {
            self.right - viewport_width + fade_width
        } else {
            offset
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HiddenWindowEdges {
    left: bool,
    right: bool,
}

impl HiddenWindowEdges {
    fn of(offset: f32, content_width: f32, viewport_width: f32) -> Self {
        let max_offset = (content_width - viewport_width).max(0.0);
        let scrolled = offset.clamp(0.0, max_offset);
        Self {
            left: scrolled > OVERFLOW_TOLERANCE_PX,
            right: max_offset - scrolled > OVERFLOW_TOLERANCE_PX,
        }
    }
}

pub fn window_list_viewport<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    offset: f32,
    focused_index: Option<usize>,
    edge_fade: WindowListEdgeFade,
    on_scroll: impl Fn(f32) -> Message + 'a,
    on_metrics: impl Fn(WindowListMetrics) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(WindowListViewport {
        content: content.into(),
        offset,
        focused_index,
        edge_fade,
        on_scroll: Box::new(on_scroll),
        on_metrics: Box::new(on_metrics),
    })
}

struct WindowListViewport<'a, Message> {
    content: Element<'a, Message>,
    offset: f32,
    focused_index: Option<usize>,
    edge_fade: WindowListEdgeFade,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
    on_metrics: Box<dyn Fn(WindowListMetrics) -> Message + 'a>,
}

#[derive(Default)]
struct State {
    last_metrics: Option<WindowListMetrics>,
    last_focus: Option<usize>,
}

impl<Message> WindowListViewport<'_, Message> {
    fn scrolled_origin(&self, viewport_width: f32, natural_width: f32) -> Point {
        let max_offset = (natural_width - viewport_width).max(0.0);
        Point::new(-self.offset.clamp(0.0, max_offset), 0.0)
    }
}

fn visible_strip(bounds: Rectangle, viewport: &Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x,
        width: bounds.width,
        y: viewport.y,
        height: viewport.height,
    }
}

fn natural_width(layout: Layout<'_>) -> f32 {
    layout.children().next().unwrap().bounds().width
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for WindowListViewport<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let budget = limits.max().width;
        let natural_limits = Limits::new(Size::ZERO, Size::new(f32::INFINITY, limits.max().height));
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &natural_limits);
        let natural = child.size();
        let width = natural.width.min(budget);
        let origin = self.scrolled_origin(width, natural.width);
        Node::with_children(
            Size::new(width, natural.height),
            vec![child.move_to(origin)],
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
        let bounds = layout.bounds();
        let Some(content_clip) = visible_strip(bounds, viewport).intersection(viewport) else {
            return;
        };
        renderer.with_layer(content_clip, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                clip_cursor(cursor, bounds),
                &content_clip,
            );
        });
        let hidden_edges = HiddenWindowEdges::of(self.offset, natural_width(layout), bounds.width);
        renderer.with_layer(content_clip, |renderer| {
            if hidden_edges.left {
                self.edge_fade.draw_at_left(renderer, theme, bounds);
            }
            if hidden_edges.right {
                self.edge_fade.draw_at_right(renderer, theme, bounds);
            }
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
        let bounds = layout.bounds();
        let child_layout = layout.children().next().unwrap();
        let natural = natural_width(layout);
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            child_layout,
            clip_cursor(cursor, bounds),
            renderer,
            clipboard,
            shell,
            &visible_strip(bounds, viewport),
        );

        match event {
            Event::Window(window::Event::RedrawRequested(_)) => {
                self.report(tree, bounds, child_layout, shell);
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if !shell.is_event_captured() && cursor.position_over(bounds).is_some() =>
            {
                let max_offset = (natural - bounds.width).max(0.0);
                if max_offset <= 0.0 {
                    return;
                }
                let travel = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => {
                        (if x == 0.0 { y } else { x }) * SCROLL_LINE_STEP_PX
                    }
                    mouse::ScrollDelta::Pixels { x, y } => {
                        if x == 0.0 {
                            y
                        } else {
                            x
                        }
                    }
                };
                let next = (self.offset - travel).clamp(0.0, max_offset);
                if (next - self.offset).abs() > f32::EPSILON {
                    shell.publish((self.on_scroll)(next));
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            _ => {}
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
        let bounds = layout.bounds();
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            clip_cursor(cursor, bounds),
            &visible_strip(bounds, viewport),
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<Message> WindowListViewport<'_, Message> {
    fn report(
        &self,
        tree: &mut Tree,
        bounds: Rectangle,
        child_layout: Layout<'_>,
        shell: &mut Shell<'_, Message>,
    ) {
        let viewport_width = bounds.width;
        let content = child_layout.bounds();
        let content_width = content.width;
        let metrics = WindowListMetrics {
            viewport_width,
            content_width,
        };
        let state = tree.state.downcast_mut::<State>();
        if state.last_metrics.as_ref() != Some(&metrics) {
            state.last_metrics = Some(metrics);
            shell.publish((self.on_metrics)(metrics));
        }

        let max_offset = (content_width - viewport_width).max(0.0);
        if let Some(focused_position) = self.focused_index
            && state.last_focus != Some(focused_position)
            && max_offset > 0.0
            && let Some(focused_node) = child_layout.children().nth(focused_position)
        {
            let focused_bounds = focused_node.bounds();
            let left = focused_bounds.x - content.x;
            let focused_span = FocusedWindowSpan {
                left,
                right: left + focused_bounds.width,
            };
            let target = focused_span
                .offset_clear_of_fades(
                    self.offset,
                    viewport_width,
                    self.edge_fade.band_width(viewport_width),
                )
                .clamp(0.0, max_offset);
            state.last_focus = Some(focused_position);
            if (target - self.offset).abs() > 0.5 {
                shell.publish((self.on_scroll)(target));
            }
        } else if let Some(focused_position) = self.focused_index {
            state.last_focus = Some(focused_position);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusedWindowSpan, HiddenWindowEdges};

    #[test]
    fn focused_window_is_scrolled_clear_of_the_fades() {
        let offset_for = |left, right, offset| {
            FocusedWindowSpan { left, right }.offset_clear_of_fades(offset, 300.0, 50.0)
        };
        let close_to = |first: f32, second: f32| (first - second).abs() < f32::EPSILON;
        assert!(close_to(offset_for(120.0, 180.0, 0.0), 0.0));
        assert!(close_to(offset_for(120.0, 180.0, 100.0), 70.0));
        assert!(close_to(offset_for(240.0, 290.0, 0.0), 40.0));
    }

    #[test]
    fn only_edges_with_hidden_windows_are_faded() {
        let hidden = |offset, content_width, viewport_width| {
            let edges = HiddenWindowEdges::of(offset, content_width, viewport_width);
            [edges.left, edges.right]
        };
        assert_eq!(hidden(0.0, 80.0, 100.0), [false, false]);
        assert_eq!(hidden(0.0, 300.0, 100.0), [false, true]);
        assert_eq!(hidden(50.0, 300.0, 100.0), [true, true]);
        assert_eq!(hidden(500.0, 300.0, 100.0), [true, false]);
    }

    #[test]
    fn subpixel_overflow_draws_no_fade() {
        let edges = HiddenWindowEdges::of(0.0, 100.25, 100.0);
        assert_eq!([edges.left, edges.right], [false, false]);
    }
}
