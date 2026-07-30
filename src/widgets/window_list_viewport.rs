//! Horizontal scroll viewport for the window list — odin's
//! `HorizontalListPager` ported to iced. Given a definite width from its
//! parent, it shows the content centred (when it fits) or clips and
//! horizontally scrolls it (when it overflows): mouse-wheel driven,
//! stop-at-bounds, auto-scrolling to keep the focused window visible.
//!
//! The scroll offset is owned by the caller, not the widget: the widget renders
//! at the offset it is handed and publishes a new one (via `on_scroll`) on
//! wheel and on focus change. iced has no per-pixel alpha mask, so the
//! odin-style edge-fade is instead baked per-item into the buttons the caller
//! builds — for which the caller needs each item's on-screen position. The
//! widget measures that and reports it back through `on_metrics` (deduped, on
//! redraw frames, as [`iced::widget::Sensor`] reports its size), and
//! [`Metrics::item_alpha`] turns it into the per-item opacity.
//!
//! Modelled on [`super::reveal`] for the clip mechanics, but stateful.

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
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

/// Line-scroll events carry no pixel travel, so scale each detent to a usable
/// step; pixel (touchpad) events already are pixels.
const LINE_STEP_PX: f32 = 60.0;

/// Opacity of a window the overflow partially hides. Any clipped item drops to
/// this fixed dim regardless of how much is hidden; fully-visible items stay
/// `1.0`.
const CLIPPED_ALPHA: f32 = 0.4;

/// The viewport geometry the caller needs to fade the items straddling the clip
/// edges: the visible width, the full content width, and each item's
/// `[left, right]` span in content-space (same order as the buttons). Reported
/// back by the widget and consumed via [`Metrics::item_alpha`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    viewport_width: f32,
    content_width: f32,
    item_bounds: Vec<(f32, f32)>,
}

impl Metrics {
    /// Largest in-bounds scroll offset for this content.
    pub fn max_scroll(&self) -> f32 {
        (self.content_width - self.viewport_width).max(0.0)
    }

    /// Opacity for item `index` at scroll `offset`: `1.0` when the item is
    /// fully in view, a fixed [`CLIPPED_ALPHA`] the moment either edge
    /// clips it. Symmetric — it tests both edges, so a window clipped on
    /// the left dims like one clipped on the right.
    pub fn item_alpha(&self, offset: f32, index: usize) -> f32 {
        let Some(&(left, right)) = self.item_bounds.get(index) else {
            return 1.0;
        };
        let max = self.max_scroll();
        if max <= 0.5 {
            return 1.0;
        }
        let offset = offset.clamp(0.0, max);
        let clipped = left < offset - 0.5 || right > offset + self.viewport_width + 0.5;
        if clipped { CLIPPED_ALPHA } else { 1.0 }
    }
}

/// Horizontally scrollable viewport around the window list. `offset` is the
/// caller-owned scroll position to render at; `focused_index` is the button of
/// the focused window (auto-scrolled into view on change); `on_scroll` requests
/// a new offset (wheel / auto-scroll) and `on_metrics` reports measured
/// geometry for the caller's per-item fade.
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
    /// Last geometry published, so redraw frames only republish on change.
    last_metrics: Option<Metrics>,
    /// Focused item last auto-scrolled to, so we snap only when focus moves.
    last_focus: Option<usize>,
}

impl<Message> WindowListViewport<'_, Message> {
    /// The on-screen translation applied to the content this frame, and whether
    /// it fits without scrolling. When it fits the child node equals its
    /// natural width, so this is a no-op shift; when it overflows the
    /// content is shifted left by the (clamped) scroll offset.
    fn shift(&self, bounds: Rectangle, natural: f32) -> Vector {
        if natural <= bounds.width {
            Vector::new((bounds.width - natural) / 2.0, 0.0)
        } else {
            let offset = self.offset.clamp(0.0, natural - bounds.width);
            Vector::new(-offset, 0.0)
        }
    }
}

/// Natural (unclipped) content width, read from the child node laid out at its
/// full width in [`WindowListViewport::layout`].
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
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        // Shrink so the content sits at its natural width and a parent can centre
        // the launcher+list pair, until it hits the width budget below and clips.
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        // Measure the child at its natural width, then take the smaller of that
        // and the budget the parent allows: at natural width when it fits, capped
        // (clipping the overflow) when it doesn't. The child stays at local x = 0
        // and overflows to the right, where `draw` clips and translates it.
        let budget = limits.max().width;
        let natural_limits = Limits::new(Size::ZERO, Size::new(f32::INFINITY, limits.max().height));
        let child =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &natural_limits);
        let natural = child.size();
        let width = natural.width.min(budget);
        Node::with_children(Size::new(width, natural.height), vec![child])
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
        let child_layout = layout.children().next().unwrap();
        let d = self.shift(bounds, natural_width(layout));
        let clip = Rectangle {
            x: bounds.x,
            width: bounds.width,
            y: viewport.y,
            height: viewport.height,
        };
        let child_cursor = clip_cursor(cursor, bounds) - d;
        let child_viewport = Rectangle {
            x: bounds.x - d.x,
            width: bounds.width,
            y: viewport.y,
            height: viewport.height,
        };
        renderer.with_layer(clip, |renderer| {
            renderer.with_translation(d, |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    child_layout,
                    child_cursor,
                    &child_viewport,
                );
            });
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
        let d = self.shift(bounds, natural);
        let child_viewport = Rectangle {
            x: bounds.x - d.x,
            width: bounds.width,
            y: viewport.y,
            height: viewport.height,
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            child_layout,
            clip_cursor(cursor, bounds) - d,
            renderer,
            clipboard,
            shell,
            &child_viewport,
        );

        match event {
            // Report geometry and follow focus on redraw frames (no user event
            // needed), deduped so a steady layout stays quiet — the same trick
            // `Sensor` uses to measure its content.
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
                // Prefer the horizontal component when present, else steer with
                // vertical wheel travel (odin's `angleDelta.y || .x`); scroll away
                // (down/right) advances into the list.
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
        let d = self.shift(bounds, natural_width(layout));
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            clip_cursor(cursor, bounds) - d,
            viewport,
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
        let bounds = layout.bounds();
        let d = self.shift(bounds, natural_width(layout));
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation + d,
        )
    }
}

impl<Message> WindowListViewport<'_, Message> {
    /// On a redraw frame: publish measured geometry when it changes, and
    /// request a new scroll offset when focus has moved to an item that is
    /// off-view.
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
        // Button bounds arrive in absolute screen space; the scroll offset lives
        // in content space (0 = content start), so rebase onto the content origin.
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
