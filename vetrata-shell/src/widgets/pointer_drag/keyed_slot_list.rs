use std::{collections::HashMap, hash::Hash, mem};

use iced::{
    Animation, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    animation::Easing,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use vetrata_config::Config;

use crate::desktop_shell::monitor_surfaces::Position;

pub fn keyed_slot_list<'a, Message: 'a, Key: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (Key, Element<'a, Message>)>,
    edge: Position,
    config: &Config,
) -> KeyedSlotList<'a, Message, Key> {
    let (keys, slots) = slots.into_iter().unzip();
    KeyedSlotList {
        keys,
        slots,
        along_vertical_edge: edge.is_vertical(),
        spacing: 0.0,
        motion: config.panel_bar.focus_indicator_motion,
    }
}

pub struct KeyedSlotList<'a, Message, Key> {
    keys: Vec<Key>,
    slots: Vec<Element<'a, Message>>,
    along_vertical_edge: bool,
    spacing: f32,
    motion: Duration,
}

impl<Message, Key> KeyedSlotList<'_, Message, Key> {
    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }
}

struct State<Key> {
    keys: Vec<Key>,
    slot_starts: HashMap<Key, Animation<f32>>,
}

impl<Key: Copy + Eq + Hash> State<Key> {
    fn is_sliding(&self, now: Instant) -> bool {
        self.slot_starts
            .values()
            .any(|start| start.is_animating(now))
    }
}

impl<Message, Key: Copy + Eq + Hash + 'static> KeyedSlotList<'_, Message, Key> {
    fn sliding_start(&self, state: &mut State<Key>, key: Key, target: f32, now: Instant) -> f32 {
        let start = state.slot_starts.entry(key).or_insert_with(|| {
            Animation::new(target)
                .easing(Easing::EaseOutCubic)
                .duration(self.motion)
        });
        if (start.value() - target).abs() > f32::EPSILON {
            start.go_mut(target, now);
        }
        start.interpolate_with(|value| value, now)
    }

    fn laid_out_slots(
        &mut self,
        trees: &mut [Tree],
        renderer: &iced::Renderer,
        limits: &Limits,
    ) -> Vec<Node> {
        if self.along_vertical_edge {
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
            let along_bar = Limits::new(Size::ZERO, Size::new(f32::INFINITY, height));
            self.slots
                .iter_mut()
                .zip(trees)
                .map(|(slot, tree)| {
                    let node = slot.as_widget_mut().layout(tree, renderer, &along_bar);
                    let centred = (height - node.size().height).max(0.0) / 2.0;
                    node.move_to((0.0, centred))
                })
                .collect()
        }
    }

    fn extent(&self, node: &Node) -> f32 {
        if self.along_vertical_edge {
            node.size().height
        } else {
            node.size().width
        }
    }

    fn placed_at(&self, node: Node, start: f32) -> Node {
        if self.along_vertical_edge {
            node.move_to((0.0, start))
        } else {
            let across = node.bounds().y;
            node.move_to((start, across))
        }
    }
}

impl<Message, Key: Copy + Eq + Hash + 'static> Widget<Message, iced::Theme, iced::Renderer>
    for KeyedSlotList<'_, Message, Key>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Key>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            keys: self.keys.clone(),
            slot_starts: HashMap::new(),
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
        if self.along_vertical_edge {
            Size::new(Length::Shrink, Length::Shrink)
        } else {
            Size::new(Length::Shrink, Length::Fill)
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let nodes = self.laid_out_slots(&mut tree.children, renderer, limits);
        let state = tree.state.downcast_mut::<State<Key>>();
        let now = Instant::now();
        let (placed, end) = self.keys.clone().into_iter().zip(nodes).fold(
            (Vec::new(), 0.0),
            |(mut placed, target), (key, node)| {
                let next = target + self.extent(&node) + self.spacing;
                let start = self.sliding_start(state, key, target, now);
                placed.push(self.placed_at(node, start));
                (placed, next)
            },
        );
        let along = (end - self.spacing).max(0.0);
        let size = if self.along_vertical_edge {
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && tree.state.downcast_ref::<State<Key>>().is_sliding(*now)
        {
            shell.invalidate_layout();
            shell.request_redraw();
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
        for ((slot, tree), layout) in self.slots.iter().zip(&tree.children).zip(layout.children()) {
            slot.as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
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
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
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

impl<'a, Message: 'a, Key: Copy + Eq + Hash + 'static> From<KeyedSlotList<'a, Message, Key>>
    for Element<'a, Message>
{
    fn from(list: KeyedSlotList<'a, Message, Key>) -> Self {
        Element::new(list)
    }
}
