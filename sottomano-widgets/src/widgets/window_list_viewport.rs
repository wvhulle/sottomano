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

use super::reveal::clip_cursor;

const LINE_STEP_PX: f32 = 60.0;

const CLIPPED_OPACITY: f32 = 0.4;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    viewport_width: f32,
    content_width: f32,
    item_bounds: Vec<(f32, f32)>,
}

impl Metrics {
    #[must_use]
    pub fn max_scroll(&self) -> f32 {
        (self.content_width - self.viewport_width).max(0.0)
    }

    #[must_use]
    pub fn item_opacity(&self, offset: f32, index: usize) -> f32 {
        let Some(&(left, right)) = self.item_bounds.get(index) else {
            return 1.0;
        };
        let max = self.max_scroll();
        if max <= 0.5 {
            return 1.0;
        }
        let offset = offset.clamp(0.0, max);
        let clipped = left < offset - 0.5 || right > offset + self.viewport_width + 0.5;
        if clipped { CLIPPED_OPACITY } else { 1.0 }
    }
}

pub fn window_list_viewport<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    offset: f32,
    focused_index: Option<usize>,
    on_scroll: impl Fn(f32) -> Message + 'a,
    on_metrics: impl Fn(Metrics) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(WindowListViewport {
        content: content.into(),
        offset,
        focused_index,
        on_scroll: Box::new(on_scroll),
        on_metrics: Box::new(on_metrics),
    })
}

struct WindowListViewport<'a, Message> {
    content: Element<'a, Message>,
    offset: f32,
    focused_index: Option<usize>,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
    on_metrics: Box<dyn Fn(Metrics) -> Message + 'a>,
}

#[derive(Default)]
struct State {
    last_metrics: Option<Metrics>,
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
        let clip = visible_strip(bounds, viewport);
        renderer.with_layer(clip, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.children().next().unwrap(),
                clip_cursor(cursor, bounds),
                &clip,
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
                        (if x == 0.0 { y } else { x }) * LINE_STEP_PX
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
        let item_bounds: Vec<(f32, f32)> = child_layout
            .children()
            .map(|node| {
                let b = node.bounds();
                (b.x - content.x, b.x + b.width - content.x)
            })
            .collect();
        let metrics = Metrics {
            viewport_width,
            content_width,
            item_bounds,
        };
        let state = tree.state.downcast_mut::<State>();
        if state.last_metrics.as_ref() != Some(&metrics) {
            state.last_metrics = Some(metrics.clone());
            shell.publish((self.on_metrics)(metrics));
        }

        let max_offset = (content_width - viewport_width).max(0.0);
        if let Some(i) = self.focused_index
            && state.last_focus != Some(i)
            && max_offset > 0.0
            && let Some(node) = child_layout.children().nth(i)
        {
            let b = node.bounds();
            let (left, right) = (b.x - content.x, b.x + b.width - content.x);
            let target = if left < self.offset {
                left
            } else if right > self.offset + viewport_width {
                right - viewport_width
            } else {
                self.offset
            }
            .clamp(0.0, max_offset);
            state.last_focus = Some(i);
            if (target - self.offset).abs() > 0.5 {
                shell.publish((self.on_scroll)(target));
            }
        } else if let Some(i) = self.focused_index {
            state.last_focus = Some(i);
        }
    }
}
