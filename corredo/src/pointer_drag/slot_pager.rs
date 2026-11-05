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

use crate::{
    list_end_corners::ListEndCorners,
    pointer_drag::{
        DragEvent, DragView,
        slot_group::{SlotGroup, SlotGroupFrame, SlotTileInset},
    },
    shared_metrics::{ACCENT_STROKE_PX, pill_corners},
    theme_colors::ThemedColor,
};

const BACKDROP_TINT_ALPHA: f32 = 0.3;
const DEFAULT_INDICATOR_MOTION: Duration = Duration::from_millis(160);
const DEFAULT_GROUP_GAP_PX: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl PagerEdge {
    fn is_vertical(self) -> bool {
        matches!(self, PagerEdge::Left | PagerEdge::Right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UngroupedHighlightLook {
    EdgeBand,
    AccentTile(SlotTileInset),
}

pub fn slot_pager<'a, Message: 'a, Key: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (Key, Element<'a, Message>)>,
    edge: PagerEdge,
) -> SlotPager<'a, Message, Key> {
    let (keys, slots) = slots.into_iter().unzip();
    SlotPager {
        keys,
        slots,
        edge,
        spacing: 0.0,
        motion: DEFAULT_INDICATOR_MOTION,
        indicator_thickness: ACCENT_STROKE_PX,
        highlight: None,
        ungrouped_highlight_look: UngroupedHighlightLook::EdgeBand,
        alarms: Vec::new(),
        groups: Vec::new(),
        group_gap: DEFAULT_GROUP_GAP_PX,
        frame_corners: Radius::default(),
        left_list_message: None,
    }
}

pub struct SlotAlarm<'a> {
    pub position: usize,
    pub color: ThemedColor<'a>,
}

struct SlotHighlight<'a, Key> {
    key: Key,
    color: ThemedColor<'a>,
}

pub struct SlotPager<'a, Message, Key> {
    keys: Vec<Key>,
    slots: Vec<Element<'a, Message>>,
    edge: PagerEdge,
    spacing: f32,
    motion: Duration,
    indicator_thickness: f32,
    highlight: Option<SlotHighlight<'a, Key>>,
    ungrouped_highlight_look: UngroupedHighlightLook,
    alarms: Vec<SlotAlarm<'a>>,
    groups: Vec<SlotGroup<'a, Message>>,
    group_gap: f32,
    frame_corners: Radius,
    left_list_message: Option<Message>,
}

impl<'a, Message, Key> SlotPager<'a, Message, Key> {
    #[must_use]
    pub fn indicator_motion(self, motion: Duration) -> Self {
        Self { motion, ..self }
    }

    #[must_use]
    pub fn group_gap(self, group_gap: f32) -> Self {
        Self { group_gap, ..self }
    }

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
    pub fn ungrouped_highlight_look(mut self, look: UngroupedHighlightLook) -> Self {
        self.ungrouped_highlight_look = look;
        self
    }

    #[must_use]
    pub fn alarms(mut self, alarms: Vec<SlotAlarm<'a>>) -> Self {
        self.alarms = alarms;
        self
    }

    #[must_use]
    pub fn groups(mut self, groups: Vec<SlotGroup<'a, Message>>) -> Self {
        self.groups = groups;
        self
    }

    #[must_use]
    pub fn frame_corners(mut self, frame_corners: Radius) -> Self {
        self.frame_corners = frame_corners;
        self
    }

    #[must_use]
    pub fn draggable<Item: Copy + PartialEq, List, Click>(
        mut self,
        list: List,
        drag: &DragView<Item>,
        on_drag: impl Fn(DragEvent<Item, List, Click>) -> Message,
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

fn edge_band(edge: PagerEdge, slots: Rectangle, slot: Rectangle, thickness: f32) -> Rectangle {
    match edge {
        PagerEdge::Bottom => Rectangle {
            height: thickness,
            ..slot
        },
        PagerEdge::Top => Rectangle {
            y: slot.y + slot.height - thickness,
            height: thickness,
            ..slot
        },
        PagerEdge::Left => Rectangle {
            x: slots.x + slots.width - thickness,
            width: thickness,
            ..slot
        },
        PagerEdge::Right => Rectangle {
            x: slots.x,
            width: thickness,
            ..slot
        },
    }
}

fn band_clear_of_corners(edge: PagerEdge, band: Rectangle, corners: Radius) -> Rectangle {
    match edge {
        PagerEdge::Left => Rectangle {
            y: band.y + corners.top_right,
            height: (band.height - corners.top_right - corners.bottom_right).max(0.0),
            ..band
        },
        PagerEdge::Right => Rectangle {
            y: band.y + corners.top_left,
            height: (band.height - corners.top_left - corners.bottom_left).max(0.0),
            ..band
        },
        PagerEdge::Bottom => Rectangle {
            x: band.x + corners.top_left,
            width: (band.width - corners.top_left - corners.top_right).max(0.0),
            ..band
        },
        PagerEdge::Top => Rectangle {
            x: band.x + corners.bottom_left,
            width: (band.width - corners.bottom_left - corners.bottom_right).max(0.0),
            ..band
        },
    }
}

fn edge_backdrop(edge: PagerEdge, slots: Rectangle, slot: Rectangle) -> Rectangle {
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

    fn laid_out_headings(
        &mut self,
        trees: &mut [Tree],
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Vec<Node> {
        let along_vertical_edge = self.along_vertical_edge();
        let height = limits.max().height;
        let heading_limits = if along_vertical_edge {
            Limits::new(Size::ZERO, limits.max())
        } else {
            Limits::new(Size::ZERO, Size::new(f32::INFINITY, height))
        };
        self.groups
            .iter_mut()
            .zip(trees)
            .map(|(group, tree)| {
                let mut node =
                    group
                        .heading
                        .as_widget_mut()
                        .layout(tree, renderer, &heading_limits);
                if !along_vertical_edge {
                    node.align_mut(Alignment::Start, Alignment::Center, Size::new(0.0, height));
                }
                node
            })
            .collect()
    }

    fn group_frame(&self, group_index: usize, layout: Layout<'_>) -> Option<SlotGroupFrame> {
        let group = self.groups.get(group_index)?;
        let heading = layout.children().nth(self.slots.len() + group_index)?;
        let slots = layout
            .children()
            .skip(group.positions.start)
            .take(group.positions.len())
            .map(|slot| slot.bounds())
            .reduce(|covered, slot| covered.union(&slot))?;
        Some(SlotGroupFrame::around_slots(
            self.along_vertical_edge(),
            heading.bounds(),
            slots,
            SlotTileInset {
                thickness: group.enclosure_thickness,
                enclosure_radius: group.enclosure_radius,
            },
        ))
    }

    fn group_index_at(&self, slot_position: usize) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.positions.contains(&slot_position))
    }

    fn highlighted_group_index(&self) -> Option<usize> {
        self.group_index_at(self.highlighted_position()?)
    }

    fn alarm_tile(&self, alarm: &SlotAlarm<'_>, layout: Layout<'_>) -> Option<renderer::Quad> {
        let slot = layout.children().nth(alarm.position)?.bounds();
        match self.group_index_at(alarm.position) {
            Some(group_index) => Some(
                self.group_frame(group_index, layout)?
                    .highlight_inside(slot),
            ),
            None => match self.ungrouped_highlight_look {
                UngroupedHighlightLook::EdgeBand => None,
                UngroupedHighlightLook::AccentTile(tile_inset) => {
                    Some(tile_inset.tile_on_slot(self.along_vertical_edge(), slot))
                }
            },
        }
    }

    fn ungrouped_highlight(&self, slots_bounds: Rectangle, slot: Rectangle) -> renderer::Quad {
        renderer::Quad {
            bounds: edge_backdrop(self.edge, slots_bounds, slot),
            border: Border {
                radius: self.highlighted_corners(),
                ..Border::default()
            },
            snap: false,
            ..renderer::Quad::default()
        }
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
        self.slots
            .iter()
            .chain(self.groups.iter().map(|group| &group.heading))
            .map(Tree::new)
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let Tree {
            state, children, ..
        } = tree;
        let state = state.downcast_mut::<State<Key>>();
        let mut previous_headings = children
            .split_off(state.keys.len().min(children.len()))
            .into_iter();
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
        children.extend(
            self.groups
                .iter()
                .map(|group| match previous_headings.next() {
                    Some(mut kept) => {
                        kept.diff(group.heading.as_widget());
                        kept
                    }
                    None => Tree::new(&group.heading),
                }),
        );
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
        let (slot_trees, heading_trees) = tree.children.split_at_mut(self.slots.len());
        let nodes = self.laid_out_slots(slot_trees, renderer, limits);
        let heading_nodes = self.laid_out_headings(heading_trees, renderer, limits);
        let state = tree.state.downcast_mut::<State<Key>>();
        let mut placed = Vec::with_capacity(nodes.len() + heading_nodes.len());
        let mut heading_starts = Vec::with_capacity(heading_nodes.len());
        let mut target = 0.0;
        for (position, (key, node)) in self.keys.clone().into_iter().zip(nodes).enumerate() {
            let leading_extent = self
                .groups
                .iter()
                .zip(&heading_nodes)
                .find(|(group, _)| group.positions.start == position)
                .map(|(group, heading)| self.extent(heading) + group.enclosure_thickness);
            target += leading_extent.unwrap_or(0.0);
            let extent = self.extent(&node);
            let start = self.sliding_start(state, key, target);
            heading_starts.extend(leading_extent.map(|leading_extent| start - leading_extent));
            placed.push(self.placed_at(node, start));
            let trailing_extent = self
                .groups
                .iter()
                .find(|group| group.positions.end == position + 1)
                .map_or(0.0, |group| group.enclosure_thickness + self.group_gap);
            target += extent + self.spacing + trailing_extent;
        }
        placed.extend(
            heading_nodes
                .into_iter()
                .zip(heading_starts)
                .map(|(heading, start)| self.placed_at(heading, start)),
        );
        let along = (target - self.spacing).max(0.0);
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
        for (group_index, group) in self.groups.iter().enumerate() {
            if let Some(frame) = self.group_frame(group_index, layout) {
                frame.draw(
                    renderer,
                    (group.enclosure_color)(theme),
                    (group.inner_color)(theme),
                );
            }
        }
        let highlighted = self
            .highlight
            .as_ref()
            .zip(self.travelled_highlight(tree, layout));
        let highlighted_group_index = self.highlighted_group_index();
        if let Some((highlight, slot)) = highlighted {
            let enclosing_group = highlighted_group_index.and_then(|group_index| {
                self.group_frame(group_index, layout)
                    .zip(self.groups.get(group_index))
            });
            let resting_slot = self.highlighted_slot(layout).unwrap_or(slot);
            match enclosing_group {
                Some((frame, group)) => {
                    renderer.fill_quad(
                        frame.highlight_inside(resting_slot),
                        (group.highlight_color)(theme),
                    );
                }
                None => match self.ungrouped_highlight_look {
                    UngroupedHighlightLook::EdgeBand => {
                        renderer.fill_quad(
                            self.ungrouped_highlight(slots_bounds, slot),
                            (highlight.color)(theme).scale_alpha(BACKDROP_TINT_ALPHA),
                        );
                    }
                    UngroupedHighlightLook::AccentTile(tile_inset) => {
                        renderer.fill_quad(
                            tile_inset.tile_on_slot(self.along_vertical_edge(), resting_slot),
                            (highlight.color)(theme),
                        );
                    }
                },
            }
        }
        for alarm in &self.alarms {
            if let Some(alarm_tile) = self.alarm_tile(alarm, layout) {
                renderer.fill_quad(alarm_tile, (alarm.color)(theme));
            }
        }
        for ((slot, tree), layout) in self.slots.iter().zip(&tree.children).zip(layout.children()) {
            slot.as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        let heading_layouts = layout.children().skip(self.slots.len());
        let heading_trees = tree.children.iter().skip(self.slots.len());
        for ((group, heading_tree), heading_layout) in
            self.groups.iter().zip(heading_trees).zip(heading_layouts)
        {
            group.heading.as_widget().draw(
                heading_tree,
                renderer,
                theme,
                style,
                heading_layout,
                cursor,
                viewport,
            );
        }
        let Some((highlight, slot)) = highlighted else {
            return;
        };
        if highlighted_group_index.is_some()
            || self.ungrouped_highlight_look != UngroupedHighlightLook::EdgeBand
        {
            return;
        }
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
