pub(super) mod scroll_motion;
mod scroll_offset_steering;
#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod selection_following_scroll_tests;

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

pub use self::scroll_motion::ScrollPacing;
use self::{
    scroll_motion::ScrollMotion,
    scroll_offset_steering::{FollowedSelection, PointerButton, ScrollerState, content_layout},
};
use crate::{
    orientation::Axis,
    selection_scroll::{
        edge_alpha_fade::{EdgeOverflow, HiddenScrollEdges, ScrollEdgeFade},
        wheel_step_direction::SCROLL_LINE_STEP_PX,
    },
    theming::FrameStyle,
    transitions::clip_cursor,
};

const NEGLIGIBLE_SCROLL_PX: f32 = 0.5;
const DEFAULT_FADE_EXTENT_PX: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelAxes {
    #[default]
    AlongOrVertical,
    AlongOnly,
}

enum ScrollOwnership<'view, Message> {
    SelfOwned,
    Controlled {
        offset: f32,
        on_scroll: Box<dyn Fn(f32) -> Message + 'view>,
    },
}

pub struct SelectionFollowingScroll<'view, Message> {
    content: Element<'view, Message>,
    axis: Axis,
    edge_fade: ScrollEdgeFade,
    pacing: ScrollPacing,
    wheel_line_travel_px: f32,
    wheel_axes: WheelAxes,
    ownership: ScrollOwnership<'view, Message>,
    selection: ScrollSelection,
    content_identity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollSelection {
    Unselected,
    SelectedAt(usize),
}

pub fn selection_following_scroll<'view, Message: 'view>(
    content: impl Into<Element<'view, Message>>,
    axis: Axis,
) -> SelectionFollowingScroll<'view, Message> {
    SelectionFollowingScroll {
        content: content.into(),
        axis,
        edge_fade: ScrollEdgeFade::away_from_frame(FrameStyle::default(), DEFAULT_FADE_EXTENT_PX),
        pacing: ScrollPacing::Instant,
        wheel_line_travel_px: SCROLL_LINE_STEP_PX,
        wheel_axes: WheelAxes::default(),
        ownership: ScrollOwnership::SelfOwned,
        selection: ScrollSelection::Unselected,
        content_identity: 0,
    }
}

impl<'view, Message: 'view> SelectionFollowingScroll<'view, Message> {
    #[must_use]
    pub fn edge_fade(self, edge_fade: ScrollEdgeFade) -> Self {
        Self { edge_fade, ..self }
    }

    #[must_use]
    pub fn wheel_line_travel(self, wheel_line_travel_px: f32) -> Self {
        Self {
            wheel_line_travel_px,
            ..self
        }
    }

    #[must_use]
    pub fn wheel_axes(self, wheel_axes: WheelAxes) -> Self {
        Self { wheel_axes, ..self }
    }

    #[must_use]
    pub fn paced(self, pacing: ScrollPacing) -> Self {
        Self { pacing, ..self }
    }

    #[must_use]
    pub fn selected(self, selected_position: usize) -> Self {
        Self {
            selection: ScrollSelection::SelectedAt(selected_position),
            ..self
        }
    }

    #[must_use]
    pub fn showing(self, content_identity: u64) -> Self {
        Self {
            content_identity,
            ..self
        }
    }

    #[must_use]
    pub fn controlled(self, offset: f32, on_scroll: impl Fn(f32) -> Message + 'view) -> Self {
        Self {
            ownership: ScrollOwnership::Controlled {
                offset,
                on_scroll: Box::new(on_scroll),
            },
            ..self
        }
    }
}

impl<'view, Message: 'view> From<SelectionFollowingScroll<'view, Message>>
    for Element<'view, Message>
{
    fn from(scroller: SelectionFollowingScroll<'view, Message>) -> Self {
        Self::new(scroller)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer>
    for SelectionFollowingScroll<'_, Message>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ScrollerState>()
    }

    fn state(&self) -> tree::State {
        let initial_offset = match &self.ownership {
            ScrollOwnership::SelfOwned => 0.0,
            ScrollOwnership::Controlled { offset, .. } => *offset,
        };
        tree::State::new(ScrollerState {
            scroll_motion: ScrollMotion::resting_at(initial_offset, self.pacing),
            content_identity: self.content_identity,
            followed_selection: FollowedSelection::AwaitingJump,
            pointer_button: PointerButton::Released,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
        let state = tree.state.downcast_mut::<ScrollerState>();
        state.scroll_motion.apply_pacing(self.pacing);
        if state.content_identity != self.content_identity {
            state.content_identity = self.content_identity;
            state.scroll_motion.jump_to(0.0);
            state.followed_selection = FollowedSelection::AwaitingJump;
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let content_node = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &self.axis.unbounded_along(limits),
        );
        let natural = content_node.size();
        let visible = self.axis.visible_size(natural, limits.max());
        let max_offset = (self.axis.size_span(natural) - self.axis.size_span(visible)).max(0.0);
        let scroll_motion = &mut tree.state.downcast_mut::<ScrollerState>().scroll_motion;
        if let ScrollOwnership::Controlled { offset, .. } = &self.ownership {
            scroll_motion.set_target(*offset);
        }
        scroll_motion.settle_if_instant(max_offset);
        let drawn_offset = scroll_motion.drawn_offset.clamp(0.0, max_offset);
        Node::with_children(
            visible,
            vec![content_node.move_to(Point::ORIGIN - self.axis.along(drawn_offset))],
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
        let Some(content_clip) = self
            .axis
            .visible_strip(bounds, viewport)
            .intersection(viewport)
        else {
            return;
        };
        let state = tree.state.downcast_ref::<ScrollerState>();
        let hidden_edges = HiddenScrollEdges::clamped_scroll_overflow(
            state.scroll_motion.drawn_offset,
            self.axis.size_span(content_layout(layout).bounds().size()),
            self.axis.size_span(bounds.size()),
        );
        let rows_clip = self
            .edge_fade
            .clear_of_outline(content_clip, self.axis, hidden_edges);
        renderer.with_layer(rows_clip, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                content_layout(layout),
                state.content_cursor(self.axis, clip_cursor(cursor, bounds)),
                &rows_clip,
            );
        });
        renderer.with_layer(content_clip, |renderer| {
            if hidden_edges.start == EdgeOverflow::Overflowing {
                self.edge_fade
                    .draw_on(self.axis.start_edge(), renderer, theme, bounds);
            }
            if hidden_edges.end == EdgeOverflow::Overflowing {
                self.edge_fade
                    .draw_on(self.axis.end_edge(), renderer, theme, bounds);
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
        {
            let state = tree.state.downcast_mut::<ScrollerState>();
            match event {
                Event::Window(window::Event::RedrawRequested(now)) => {
                    self.advance_scroll(state, layout, cursor, *now, shell);
                }
                Event::Mouse(mouse::Event::ButtonPressed(_)) => {
                    state.pointer_button = PointerButton::Held;
                }
                _ => {}
            }
        }
        let content_cursor = tree
            .state
            .downcast_ref::<ScrollerState>()
            .content_cursor(self.axis, clip_cursor(cursor, bounds));
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content_layout(layout),
            content_cursor,
            renderer,
            clipboard,
            shell,
            &self.axis.visible_strip(bounds, viewport),
        );
        let state = tree.state.downcast_mut::<ScrollerState>();
        match event {
            Event::Window(window::Event::RedrawRequested(_)) => {
                self.keep_selection_clear_of_fades(state, layout, shell);
            }
            Event::Mouse(mouse::Event::ButtonReleased(_)) => {
                state.pointer_button = PointerButton::Released;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if !shell.is_event_captured() && cursor.position_over(bounds).is_some() =>
            {
                self.scroll_by_wheel(state, layout, *delta, shell);
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
            content_layout(layout),
            tree.state
                .downcast_ref::<ScrollerState>()
                .content_cursor(self.axis, clip_cursor(cursor, bounds)),
            &self.axis.visible_strip(bounds, viewport),
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
            content_layout(layout),
            renderer,
            operation,
        );
    }

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            content_layout(layout),
            renderer,
            viewport,
            translation,
        )
    }
}
