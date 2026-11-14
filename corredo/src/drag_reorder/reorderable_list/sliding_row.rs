use std::{collections::HashMap, convert::identity, hash::Hash, mem};

use iced::{
    Alignment, Animation, Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};

use crate::{
    orientation::Axis,
    transitions::{MotionState, eased_animation, motion_of},
};

pub(super) struct SlidingRow<'a, Message, RowKey> {
    keys: Vec<RowKey>,
    children: Vec<Element<'a, Message>>,
    placement_order: Vec<usize>,
    gaps_after: Vec<f32>,
    pager_axis: Axis,
    slide_duration: Duration,
}

impl<'a, Message, RowKey> SlidingRow<'a, Message, RowKey> {
    pub(super) fn new(
        keyed_children: impl IntoIterator<Item = (RowKey, Element<'a, Message>)>,
        pager_axis: Axis,
        slide_duration: Duration,
    ) -> Self {
        let (keys, children): (Vec<_>, Vec<_>) = keyed_children.into_iter().unzip();
        Self {
            placement_order: (0..children.len()).collect(),
            gaps_after: vec![0.0; children.len()],
            keys,
            children,
            pager_axis,
            slide_duration,
        }
    }

    pub(super) fn placed_in_order(mut self, placement_order: Vec<usize>) -> Self {
        self.placement_order = placement_order;
        self
    }

    pub(super) fn gaps_after(mut self, gaps_after: Vec<f32>) -> Self {
        self.gaps_after = gaps_after;
        self
    }

    fn laid_out_children(
        &mut self,
        trees: &mut [Tree],
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Vec<Node> {
        match self.pager_axis {
            Axis::Vertical => {
                let intrinsic = Limits::with_compression(
                    Size::ZERO,
                    limits.max(),
                    Size::new(true, limits.compression().height),
                );
                let widest = self
                    .children
                    .iter_mut()
                    .zip(trees.iter_mut())
                    .map(|(child, tree)| {
                        child
                            .as_widget_mut()
                            .layout(tree, renderer, &intrinsic)
                            .size()
                            .width
                    })
                    .fold(limits.min().width, f32::max);
                let stretched = Limits::new(
                    Size::new(widest, 0.0),
                    Size::new(widest, limits.max().height),
                );
                self.children
                    .iter_mut()
                    .zip(trees)
                    .map(|(child, tree)| child.as_widget_mut().layout(tree, renderer, &stretched))
                    .collect()
            }
            Axis::Horizontal => {
                let height = limits.max().height;
                let along_row = Limits::new(Size::ZERO, Size::new(f32::INFINITY, height));
                self.children
                    .iter_mut()
                    .zip(trees)
                    .map(|(child, tree)| {
                        let mut node = child.as_widget_mut().layout(tree, renderer, &along_row);
                        node.align_mut(Alignment::Start, Alignment::Center, Size::new(0.0, height));
                        node
                    })
                    .collect()
            }
        }
    }

    fn extent_along(&self, node: &Node) -> f32 {
        match self.pager_axis {
            Axis::Vertical => node.size().height,
            Axis::Horizontal => node.size().width,
        }
    }

    fn placed_at(&self, node: Node, start: f32) -> Node {
        match self.pager_axis {
            Axis::Vertical => node.move_to((0.0, start)),
            Axis::Horizontal => {
                let across = node.bounds().y;
                node.move_to((start, across))
            }
        }
    }
}

struct SlidingRowState<RowKey> {
    keys: Vec<RowKey>,
    child_starts: HashMap<RowKey, Animation<f32>>,
    frame: Option<Instant>,
}

impl<RowKey: Copy + Eq + Hash> SlidingRowState<RowKey> {
    fn slide_motion(&self, now: Instant) -> MotionState {
        motion_of(self.child_starts.values(), now)
    }

    fn sliding_start(&mut self, key: RowKey, target: f32, slide_duration: Duration) -> f32 {
        let Some(now) = self.frame else {
            return target;
        };
        let start = self
            .child_starts
            .entry(key)
            .or_insert_with(|| eased_animation(target, slide_duration));
        if (start.value() - target).abs() > f32::EPSILON {
            start.go_mut(target, now);
        }
        start.interpolate_with(identity, now)
    }
}

impl<Message, RowKey: Copy + Eq + Hash + 'static> Widget<Message, Theme, iced::Renderer>
    for SlidingRow<'_, Message, RowKey>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SlidingRowState<RowKey>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SlidingRowState {
            keys: self.keys.clone(),
            child_starts: HashMap::new(),
            frame: None,
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let Tree {
            state, children, ..
        } = tree;
        let row_state = state.downcast_mut::<SlidingRowState<RowKey>>();
        let mut previous_trees: HashMap<RowKey, Tree> = mem::take(&mut row_state.keys)
            .into_iter()
            .zip(mem::take(children))
            .collect();
        *children = self
            .keys
            .iter()
            .zip(&self.children)
            .map(|(key, child)| match previous_trees.remove(key) {
                Some(mut kept_tree) => {
                    kept_tree.diff(child.as_widget());
                    kept_tree
                }
                None => Tree::new(child),
            })
            .collect();
        row_state.keys.clone_from(&self.keys);
        row_state
            .child_starts
            .retain(|key, _| self.keys.contains(key));
    }

    fn size(&self) -> Size<Length> {
        match self.pager_axis {
            Axis::Vertical => Size::new(Length::Shrink, Length::Shrink),
            Axis::Horizontal => Size::new(Length::Shrink, Length::Fill),
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let unplaced = self.laid_out_children(&mut tree.children, renderer, limits);
        let row_state = tree.state.downcast_mut::<SlidingRowState<RowKey>>();
        let mut placed: Vec<Option<Node>> = unplaced.into_iter().map(Some).collect();
        let mut target = 0.0;
        for &child_index in &self.placement_order {
            let Some(node) = placed.get_mut(child_index).and_then(Option::take) else {
                continue;
            };
            let extent = self.extent_along(&node);
            let start =
                row_state.sliding_start(self.keys[child_index], target, self.slide_duration);
            placed[child_index] = Some(self.placed_at(node, start));
            target += extent + self.gaps_after.get(child_index).copied().unwrap_or(0.0);
        }
        let placed: Vec<Node> = placed
            .into_iter()
            .map(|node| node.unwrap_or_else(|| Node::new(Size::ZERO)))
            .collect();
        let along = target.max(0.0);
        let size = match self.pager_axis {
            Axis::Vertical => Size::new(
                placed
                    .iter()
                    .map(|node| node.size().width)
                    .fold(0.0, f32::max),
                along,
            ),
            Axis::Horizontal => Size::new(along, limits.max().height),
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
            let row_state = tree.state.downcast_mut::<SlidingRowState<RowKey>>();
            row_state.frame = Some(*now);
            if row_state.slide_motion(*now) == MotionState::Moving {
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }
        for ((child, child_tree), child_layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child.as_widget_mut().update(
                child_tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
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
        for ((child, child_tree), child_layout) in self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
        {
            child.as_widget().draw(
                child_tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
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
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, child_tree), child_layout)| {
                child.as_widget().mouse_interaction(
                    child_tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                )
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
            for ((child, child_tree), child_layout) in self
                .children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                child
                    .as_widget_mut()
                    .operate(child_tree, child_layout, renderer, operation);
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
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a, RowKey: Copy + Eq + Hash + 'static> From<SlidingRow<'a, Message, RowKey>>
    for Element<'a, Message>
{
    fn from(sliding_row: SlidingRow<'a, Message, RowKey>) -> Self {
        Element::new(sliding_row)
    }
}
