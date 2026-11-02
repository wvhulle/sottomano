use std::{collections::HashMap, convert::identity, hash::Hash, mem};

use iced::{
    Alignment, Animation, Color, Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    border::{Border, Radius},
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use sottomano_config::{Config, screen_edges::Position};

use crate::widgets::{
    configured_theme::ThemedColor,
    list_end_corners::ListEndCorners,
    pointer_drag::{DragEvent, DragView, PanelSlotList},
    shared_metrics::{ACCENT_STROKE_PX, pill_corners},
};

const BACKDROP_TINT_ALPHA: f32 = 0.3;

pub fn slot_pager<'a, Message: 'a, Key: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (Key, Element<'a, Message>)>,
    edge: Position,
    config: &Config,
) -> SlotPager<'a, Message, Key> {
    let (keys, slots) = slots.into_iter().unzip();
    SlotPager {
        keys,
        slots,
        edge,
        spacing: 0.0,
        motion: config.desktop_panels.slot_drag.focus_indicator_duration,
        indicator_thickness: ACCENT_STROKE_PX,
        highlight: None,
        frame_corners: Radius::default(),
        left_list_message: None,
    }
}

struct SlotHighlight<'a, Key> {
    key: Key,
    color: ThemedColor<'a>,
}

pub struct SlotPager<'a, Message, Key> {
    keys: Vec<Key>,
    slots: Vec<Element<'a, Message>>,
    edge: Position,
    spacing: f32,
    motion: Duration,
    indicator_thickness: f32,
    highlight: Option<SlotHighlight<'a, Key>>,
    frame_corners: Radius,
    left_list_message: Option<Message>,
}

impl<'a, Message, Key> SlotPager<'a, Message, Key> {
    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    #[must_use]
    pub fn highlight(
        mut self,
        highlighted_key: Option<Key>,
        color: impl Fn(&Theme) -> Color + 'a,
    ) -> Self {
        self.highlight = highlighted_key.map(|key| SlotHighlight {
            key,
            color: Box::new(color),
        });
        self
    }

    #[must_use]
    pub fn frame_corners(mut self, frame_corners: Radius) -> Self {
        self.frame_corners = frame_corners;
        self
    }

    #[must_use]
    pub fn draggable<Click>(
        mut self,
        list: PanelSlotList,
        drag: &DragView,
        on_drag: impl Fn(DragEvent<Click>) -> Message,
    ) -> Self {
        self.left_list_message = drag.is_active().then(|| on_drag(DragEvent::LeftList(list)));
        self
    }
}

struct State<Key> {
    keys: Vec<Key>,
    slot_starts: HashMap<Key, Animation<f32>>,
    highlight_travel: Option<HighlightTravel>,
    cursor_over_list: bool,
    frame: Option<Instant>,
}

impl<Key: Copy + Eq + Hash> State<Key> {
    fn is_sliding(&self, now: Instant) -> bool {
        self.slot_starts
            .values()
            .any(|start| start.is_animating(now))
    }
}

#[derive(Clone, Copy)]
struct TravelledSpan {
    start: f32,
    extent: f32,
}

struct HighlightTravel {
    start: Animation<f32>,
    extent: Animation<f32>,
}

fn eased_motion(value: f32, duration: Duration) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(duration)
}

impl HighlightTravel {
    fn resting_at(span: TravelledSpan, duration: Duration) -> Self {
        Self {
            start: eased_motion(span.start, duration),
            extent: eased_motion(span.extent, duration),
        }
    }

    fn aim(&mut self, span: TravelledSpan, now: Instant) {
        self.start.go_mut(span.start, now);
        self.extent.go_mut(span.extent, now);
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.start.is_animating(now) || self.extent.is_animating(now)
    }

    fn travelled(&self, now: Instant) -> TravelledSpan {
        TravelledSpan {
            start: self.start.interpolate_with(identity, now),
            extent: self.extent.interpolate_with(identity, now),
        }
    }
}

fn edge_band(edge: Position, slots: Rectangle, slot: Rectangle, thickness: f32) -> Rectangle {
    match edge {
        Position::Bottom => Rectangle {
            height: thickness,
            ..slot
        },
        Position::Top => Rectangle {
            y: slot.y + slot.height - thickness,
            height: thickness,
            ..slot
        },
        Position::Left => Rectangle {
            x: slots.x + slots.width - thickness,
            width: thickness,
            ..slot
        },
    }
}

fn band_clear_of_corners(edge: Position, band: Rectangle, corners: Radius) -> Rectangle {
    match edge {
        Position::Left => Rectangle {
            y: band.y + corners.top_right,
            height: (band.height - corners.top_right - corners.bottom_right).max(0.0),
            ..band
        },
        Position::Bottom => Rectangle {
            x: band.x + corners.top_left,
            width: (band.width - corners.top_left - corners.top_right).max(0.0),
            ..band
        },
        Position::Top => Rectangle {
            x: band.x + corners.bottom_left,
            width: (band.width - corners.bottom_left - corners.bottom_right).max(0.0),
            ..band
        },
    }
}

fn edge_backdrop(edge: Position, slots: Rectangle, slot: Rectangle) -> Rectangle {
    if edge.is_vertical() {
        Rectangle {
            x: slots.x,
            width: slots.width,
            ..slot
        }
    } else {
        Rectangle {
            y: slots.y,
            height: slots.height,
            ..slot
        }
    }
}

impl<Message, Key: Copy + Eq + Hash + 'static> SlotPager<'_, Message, Key> {
    fn along_vertical_edge(&self) -> bool {
        self.edge.is_vertical()
    }

    fn sliding_start(&self, state: &mut State<Key>, key: Key, target: f32) -> f32 {
        let Some(now) = state.frame else {
            return target;
        };
        let start = state
            .slot_starts
            .entry(key)
            .or_insert_with(|| eased_motion(target, self.motion));
        if (start.value() - target).abs() > f32::EPSILON {
            start.go_mut(target, now);
        }
        start.interpolate_with(identity, now)
    }

    fn laid_out_slots(
        &mut self,
        trees: &mut [Tree],
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Vec<Node> {
        if self.along_vertical_edge() {
            let intrinsic = Limits::with_compression(
                Size::ZERO,
                limits.max(),
                Size::new(true, limits.compression().height),
            );
            let width = self
                .slots
                .iter_mut()
                .zip(trees.iter_mut())
                .map(|(slot, tree)| {
                    slot.as_widget_mut()
                        .layout(tree, renderer, &intrinsic)
                        .size()
                        .width
                })
                .fold(limits.min().width, f32::max);
            let stretched =
                Limits::new(Size::new(width, 0.0), Size::new(width, limits.max().height));
            self.slots
                .iter_mut()
                .zip(trees)
                .map(|(slot, tree)| slot.as_widget_mut().layout(tree, renderer, &stretched))
                .collect()
        } else {
            let height = limits.max().height;
            let along_panel = Limits::new(Size::ZERO, Size::new(f32::INFINITY, height));
            self.slots
                .iter_mut()
                .zip(trees)
                .map(|(slot, tree)| {
                    let mut node = slot.as_widget_mut().layout(tree, renderer, &along_panel);
                    node.align_mut(Alignment::Start, Alignment::Center, Size::new(0.0, height));
                    node
                })
                .collect()
        }
    }

    fn extent(&self, node: &Node) -> f32 {
        if self.along_vertical_edge() {
            node.size().height
        } else {
            node.size().width
        }
    }

    fn placed_at(&self, node: Node, start: f32) -> Node {
        if self.along_vertical_edge() {
            node.move_to((0.0, start))
        } else {
            let across = node.bounds().y;
            node.move_to((start, across))
        }
    }

    fn highlighted_position(&self) -> Option<usize> {
        let highlight = self.highlight.as_ref()?;
        self.keys.iter().position(|key| *key == highlight.key)
    }

    fn highlighted_slot(&self, layout: Layout<'_>) -> Option<Rectangle> {
        Some(
            layout
                .children()
                .nth(self.highlighted_position()?)?
                .bounds(),
        )
    }

    fn highlighted_corners(&self) -> Radius {
        self.highlighted_position()
            .map_or_else(Radius::default, |highlighted_position| {
                ListEndCorners::new(self.keys.len(), self.frame_corners)
                    .radius_at(highlighted_position)
            })
    }

    fn span(&self, slot: Rectangle) -> TravelledSpan {
        if self.along_vertical_edge() {
            TravelledSpan {
                start: slot.y,
                extent: slot.height,
            }
        } else {
            TravelledSpan {
                start: slot.x,
                extent: slot.width,
            }
        }
    }

    fn travelled_slot(&self, slot: Rectangle, travelled: TravelledSpan) -> Rectangle {
        if self.along_vertical_edge() {
            Rectangle {
                y: travelled.start,
                height: travelled.extent,
                ..slot
            }
        } else {
            Rectangle {
                x: travelled.start,
                width: travelled.extent,
                ..slot
            }
        }
    }

    fn travelled_highlight(&self, tree: &Tree, layout: Layout<'_>) -> Option<Rectangle> {
        let slot = self.highlighted_slot(layout)?;
        let state = tree.state.downcast_ref::<State<Key>>();
        let travelled = state.highlight_travel.as_ref()?.travelled(state.frame?);
        Some(self.travelled_slot(slot, travelled))
    }

    fn aim_highlight(&self, state: &mut State<Key>, layout: Layout<'_>, now: Instant) -> bool {
        let Some(slot) = self.highlighted_slot(layout) else {
            return false;
        };
        let span = self.span(slot);
        let travel = state
            .highlight_travel
            .get_or_insert_with(|| HighlightTravel::resting_at(span, self.motion));
        travel.aim(span, now);
        travel.is_animating(now)
    }
}

impl<Message: Clone, Key: Copy + Eq + Hash + 'static> Widget<Message, Theme, iced::Renderer>
    for SlotPager<'_, Message, Key>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Key>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            keys: self.keys.clone(),
            slot_starts: HashMap::new(),
            highlight_travel: None,
            cursor_over_list: false,
            frame: None,
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.slots.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let Tree {
            state, children, ..
        } = tree;
        let state = state.downcast_mut::<State<Key>>();
        let mut previous: HashMap<Key, Tree> = mem::take(&mut state.keys)
            .into_iter()
            .zip(mem::take(children))
            .collect();
        *children = self
            .keys
            .iter()
            .zip(&self.slots)
            .map(|(key, slot)| match previous.remove(key) {
                Some(mut kept) => {
                    kept.diff(slot.as_widget());
                    kept
                }
                None => Tree::new(slot),
            })
            .collect();
        state.keys.clone_from(&self.keys);
        state.slot_starts.retain(|key, _| self.keys.contains(key));
    }

    fn size(&self) -> Size<Length> {
        if self.along_vertical_edge() {
            Size::new(Length::Shrink, Length::Shrink)
        } else {
            Size::new(Length::Shrink, Length::Fill)
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let nodes = self.laid_out_slots(&mut tree.children, renderer, limits);
        let state = tree.state.downcast_mut::<State<Key>>();
        let (placed, end) = self.keys.clone().into_iter().zip(nodes).fold(
            (Vec::new(), 0.0),
            |(mut placed, target), (key, node)| {
                let next = target + self.extent(&node) + self.spacing;
                let start = self.sliding_start(state, key, target);
                placed.push(self.placed_at(node, start));
                (placed, next)
            },
        );
        let along = (end - self.spacing).max(0.0);
        let size = if self.along_vertical_edge() {
            let widest = placed
                .iter()
                .map(|node| node.size().width)
                .fold(0.0, f32::max);
            Size::new(widest, along)
        } else {
            Size::new(along, limits.max().height)
        };
        Node::with_children(size, placed)
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State<Key>>();
            state.frame = Some(*now);
            if state.is_sliding(*now) {
                shell.invalidate_layout();
                shell.request_redraw();
            }
            if self.aim_highlight(state, layout, *now) {
                shell.request_redraw();
            }
        }
        for ((slot, tree), layout) in self
            .slots
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            slot.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
        if shell.is_event_captured() {
            return;
        }
        let state = tree.state.downcast_mut::<State<Key>>();
        let was_over_list =
            mem::replace(&mut state.cursor_over_list, cursor.is_over(layout.bounds()));
        if was_over_list
            && !state.cursor_over_list
            && let Some(left_list_message) = &self.left_list_message
        {
            shell.publish(left_list_message.clone());
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let slots_bounds = layout.bounds();
        let highlighted = self
            .highlight
            .as_ref()
            .zip(self.travelled_highlight(tree, layout));
        if let Some((highlight, slot)) = highlighted {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: edge_backdrop(self.edge, slots_bounds, slot),
                    border: Border {
                        radius: self.highlighted_corners(),
                        ..Border::default()
                    },
                    snap: false,
                    ..renderer::Quad::default()
                },
                (highlight.color)(theme).scale_alpha(BACKDROP_TINT_ALPHA),
            );
        }
        for ((slot, tree), layout) in self.slots.iter().zip(&tree.children).zip(layout.children()) {
            slot.as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        let Some((highlight, slot)) = highlighted else {
            return;
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: band_clear_of_corners(
                    self.edge,
                    edge_band(self.edge, slots_bounds, slot, self.indicator_thickness),
                    self.highlighted_corners(),
                ),
                border: pill_corners(self.indicator_thickness),
                snap: false,
                ..renderer::Quad::default()
            },
            (highlight.color)(theme),
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.slots
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((slot, tree), layout)| {
                slot.as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            for ((slot, tree), layout) in self
                .slots
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                slot.as_widget_mut()
                    .operate(tree, layout, renderer, operation);
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        overlay::from_children(
            &mut self.slots,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a, Key: Copy + Eq + Hash + 'static> From<SlotPager<'a, Message, Key>>
    for Element<'a, Message>
{
    fn from(pager: SlotPager<'a, Message, Key>) -> Self {
        Element::new(pager)
    }
}
