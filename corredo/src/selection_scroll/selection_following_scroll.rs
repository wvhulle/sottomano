pub(super) mod scroll_motion;

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
    time::Instant,
    window,
};

pub use self::scroll_motion::ScrollPacing;
use self::scroll_motion::{OffsetChange, ScrollFrame, ScrollMotion};
use crate::{
    orientation::Axis,
    pointer_hover::hover_over_bounds,
    selection_scroll::{
        edge_alpha_fade::{EdgeOverflow, HiddenScrollEdges, ScrollEdgeFade},
        wheel_step_direction::SCROLL_LINE_STEP_PX,
    },
    theming::FrameStyle,
    transitions::{MotionState, clip_cursor},
};

const NEGLIGIBLE_SCROLL_PX: f32 = 0.5;
const DEFAULT_FADE_EXTENT_PX: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WheelAxes {
    #[default]
    AlongOrVertical,
    AlongOnly,
}

impl Axis {
    fn wheel_travel(
        self,
        delta: mouse::ScrollDelta,
        line_travel_px: f32,
        wheel_axes: WheelAxes,
    ) -> f32 {
        let (horizontal, vertical, step) = match delta {
            mouse::ScrollDelta::Lines { x, y } => (x, y, line_travel_px),
            mouse::ScrollDelta::Pixels { x, y } => (x, y, 1.0),
        };
        let along_axis = match (self, wheel_axes) {
            (Self::Horizontal, WheelAxes::AlongOnly) => horizontal,
            (Self::Horizontal, WheelAxes::AlongOrVertical) if horizontal != 0.0 => horizontal,
            (Self::Horizontal, WheelAxes::AlongOrVertical) | (Self::Vertical, _) => vertical,
        };
        along_axis * step
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectedSpan {
    start: f32,
    end: f32,
}

impl SelectedSpan {
    fn offset_clear_of_fades(self, offset: f32, visible_span: f32, fade_extent: f32) -> f32 {
        if self.start < offset + fade_extent {
            self.start - fade_extent
        } else if self.end > offset + visible_span - fade_extent {
            self.end - visible_span + fade_extent
        } else {
            offset
        }
    }
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

struct ScrollerState {
    scroll_motion: ScrollMotion,
    content_identity: u64,
    followed_selection: FollowedSelection,
    pointer_button: PointerButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FollowedSelection {
    AwaitingJump,
    FollowedAt(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerButton {
    Released,
    Held,
}

impl ScrollerState {
    fn content_cursor(&self, axis: Axis, visible_cursor: Cursor) -> Cursor {
        match visible_cursor {
            Cursor::Available(position) if self.pointer_button == PointerButton::Released => {
                Cursor::Available(position + axis.along(self.scroll_motion.hover_drift()))
            }
            Cursor::Available(_) | Cursor::Levitating(_) | Cursor::Unavailable => visible_cursor,
        }
    }
}

fn content_layout(layout: Layout<'_>) -> Layout<'_> {
    layout.children().next().unwrap()
}

impl<Message> SelectionFollowingScroll<'_, Message> {
    fn max_offset(&self, layout: Layout<'_>) -> f32 {
        let content_span = self.axis.size_span(content_layout(layout).bounds().size());
        (content_span - self.axis.size_span(layout.bounds().size())).max(0.0)
    }

    fn scroll_to(&self, state: &mut ScrollerState, offset: f32, shell: &mut Shell<'_, Message>) {
        match &self.ownership {
            ScrollOwnership::SelfOwned => state.scroll_motion.set_target(offset),
            ScrollOwnership::Controlled { on_scroll, .. } => shell.publish(on_scroll(offset)),
        }
        shell.request_redraw();
    }

    fn advance_scroll(
        &self,
        state: &mut ScrollerState,
        layout: Layout<'_>,
        cursor: Cursor,
        now: Instant,
        shell: &mut Shell<'_, Message>,
    ) {
        let offset_change = state.scroll_motion.advance(ScrollFrame {
            now,
            max_offset: self.max_offset(layout),
            pointer_hover: hover_over_bounds(cursor, layout.bounds()),
        });
        if offset_change == OffsetChange::Moved {
            shell.invalidate_layout();
        }
        if state.scroll_motion.travel_state(now) == MotionState::Moving {
            shell.request_redraw();
        }
    }

    fn keep_selection_clear_of_fades(
        &self,
        state: &mut ScrollerState,
        layout: Layout<'_>,
        shell: &mut Shell<'_, Message>,
    ) {
        let ScrollSelection::SelectedAt(selected_position) = self.selection else {
            return;
        };
        let previously_followed = state.followed_selection;
        if previously_followed == FollowedSelection::FollowedAt(selected_position) {
            return;
        }
        state.followed_selection = FollowedSelection::FollowedAt(selected_position);
        let max_offset = self.max_offset(layout);
        let content = content_layout(layout);
        let Some(selected_node) = content.children().nth(selected_position) else {
            return;
        };
        if max_offset <= 0.0 {
            return;
        }
        let selected_bounds = selected_node.bounds();
        let start = self.axis.point_coordinate(selected_bounds.position())
            - self.axis.point_coordinate(content.bounds().position());
        let visible_span = self.axis.size_span(layout.bounds().size());
        let current = state.scroll_motion.target_offset;
        let target = SelectedSpan {
            start,
            end: start + self.axis.size_span(selected_bounds.size()),
        }
        .offset_clear_of_fades(
            current,
            visible_span,
            self.edge_fade.extent_within(visible_span),
        )
        .clamp(0.0, max_offset);
        if (target - current).abs() <= NEGLIGIBLE_SCROLL_PX {
            return;
        }
        if previously_followed == FollowedSelection::AwaitingJump {
            state.scroll_motion.jump_to(target);
            shell.invalidate_layout();
        }
        self.scroll_to(state, target, shell);
    }

    fn scroll_by_wheel(
        &self,
        state: &mut ScrollerState,
        layout: Layout<'_>,
        delta: mouse::ScrollDelta,
        shell: &mut Shell<'_, Message>,
    ) {
        let current = state.scroll_motion.target_offset;
        let next = (current
            - self
                .axis
                .wheel_travel(delta, self.wheel_line_travel_px, self.wheel_axes))
        .clamp(0.0, self.max_offset(layout));
        if (next - current).abs() <= f32::EPSILON {
            return;
        }
        if matches!(delta, mouse::ScrollDelta::Pixels { .. }) {
            state.scroll_motion.jump_to(next);
            shell.invalidate_layout();
        }
        self.scroll_to(state, next, shell);
        shell.capture_event();
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

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use iced::{
        Element, Event, Point, Settings, Size,
        advanced::widget::Tree,
        mouse::{Cursor, Event as MouseEvent, ScrollDelta},
        time::{Duration, Instant},
        widget::{Column, Row, text},
        window,
    };
    use iced_test::Simulator;

    use super::{
        FollowedSelection, PointerButton, ScrollPacing, ScrollerState, SelectedSpan,
        SelectionFollowingScroll, WheelAxes, selection_following_scroll,
    };
    use crate::{
        orientation::Axis, pointer_hover::PointerHover,
        selection_scroll::edge_alpha_fade::ScrollEdgeFade, theming::FrameStyle,
    };

    const VISIBLE_SIZE: Size = Size::new(200.0, 100.0);
    const ENTRY_COUNT: usize = 40;
    const FADE_EXTENT_PX: f32 = 20.0;

    fn edge_fade() -> ScrollEdgeFade {
        ScrollEdgeFade::away_from_frame(FrameStyle::default(), FADE_EXTENT_PX)
    }

    fn entry_label(position: usize) -> String {
        format!("entry-{position}")
    }

    fn long_content<'view>(axis: Axis) -> Element<'view, ()> {
        let entries = (0..ENTRY_COUNT).map(|position| text(entry_label(position)).into());
        match axis {
            Axis::Horizontal => Row::with_children(entries).spacing(10.0).into(),
            Axis::Vertical => Column::with_children(entries).spacing(10.0).into(),
        }
    }

    fn scroller_over<'view>(axis: Axis) -> SelectionFollowingScroll<'view, ()> {
        selection_following_scroll(long_content(axis), axis).edge_fade(edge_fade())
    }

    fn first_entry_position(simulator: &mut Simulator<'_, ()>, axis: Axis) -> f32 {
        let bounds = simulator
            .find(entry_label(0).as_str())
            .expect("the first entry is laid out")
            .bounds();
        axis.point_coordinate(bounds.position())
    }

    fn wheel_down(simulator: &mut Simulator<'_, ()>, delta: ScrollDelta) {
        simulator.point_at(Point::new(5.0, 5.0));
        let _moved = simulator.simulate([Event::Mouse(MouseEvent::CursorMoved {
            position: Point::new(5.0, 5.0),
        })]);
        let _wheeled = simulator.simulate([Event::Mouse(MouseEvent::WheelScrolled { delta })]);
        let now = Instant::now();
        let _redrawn = simulator.simulate([
            Event::Window(window::Event::RedrawRequested(now)),
            Event::Window(window::Event::RedrawRequested(now + Duration::from_secs(1))),
        ]);
    }

    #[test]
    fn a_longer_wheel_line_travels_further() {
        let travelled = |line_travel_px: f32| {
            let scroller = scroller_over(Axis::Vertical).wheel_line_travel(line_travel_px);
            let mut simulator = Simulator::with_size(Settings::default(), VISIBLE_SIZE, scroller);
            let before = first_entry_position(&mut simulator, Axis::Vertical);
            wheel_down(&mut simulator, ScrollDelta::Lines { x: 0.0, y: -1.0 });
            before - first_entry_position(&mut simulator, Axis::Vertical)
        };
        assert!(travelled(60.0) > travelled(20.0));
    }

    #[test]
    fn the_wheel_moves_content_along_either_axis() {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let mut simulator =
                Simulator::with_size(Settings::default(), VISIBLE_SIZE, scroller_over(axis));
            let before = first_entry_position(&mut simulator, axis);
            wheel_down(&mut simulator, ScrollDelta::Lines { x: 0.0, y: -2.0 });
            assert!(
                first_entry_position(&mut simulator, axis) < before,
                "{axis:?}"
            );
        }
    }

    #[test]
    fn a_horizontal_scroller_limited_to_its_axis_ignores_a_vertical_wheel() {
        let axis = Axis::Horizontal;
        let scroller = scroller_over(axis).wheel_axes(WheelAxes::AlongOnly);
        let mut simulator = Simulator::with_size(Settings::default(), VISIBLE_SIZE, scroller);
        let before = first_entry_position(&mut simulator, axis);
        wheel_down(&mut simulator, ScrollDelta::Lines { x: 0.0, y: -2.0 });
        assert!((first_entry_position(&mut simulator, axis) - before).abs() < 0.5);
        wheel_down(&mut simulator, ScrollDelta::Lines { x: -2.0, y: 0.0 });
        assert!(first_entry_position(&mut simulator, axis) < before);
    }

    #[test]
    fn eased_scrolling_arrives_at_the_same_place() {
        let axis = Axis::Vertical;
        let eased = scroller_over(axis).paced(ScrollPacing::Eased(Duration::from_millis(160)));
        let mut simulator = Simulator::with_size(Settings::default(), VISIBLE_SIZE, eased);
        let before = first_entry_position(&mut simulator, axis);
        wheel_down(&mut simulator, ScrollDelta::Lines { x: 0.0, y: -1.0 });
        let moved = before - first_entry_position(&mut simulator, axis);
        assert!(moved > 0.0, "{moved}");
    }

    #[test]
    fn scrolling_past_the_end_stays_within_the_content() {
        let axis = Axis::Vertical;
        let mut simulator =
            Simulator::with_size(Settings::default(), VISIBLE_SIZE, scroller_over(axis));
        wheel_down(
            &mut simulator,
            ScrollDelta::Pixels {
                x: 0.0,
                y: -1_000_000.0,
            },
        );
        let last_entry = simulator
            .find(entry_label(ENTRY_COUNT - 1).as_str())
            .expect("the last entry is laid out")
            .bounds();
        let last_entry_end = last_entry.y + last_entry.height;
        assert!(
            (last_entry_end - VISIBLE_SIZE.height).abs() < 1.0,
            "{last_entry_end}"
        );
    }

    #[test]
    fn a_wheel_over_one_scroller_leaves_its_sibling_alone() {
        let axis = Axis::Vertical;
        let side_by_side: Element<'_, ()> = Row::new()
            .push(scroller_over(axis))
            .push(
                selection_following_scroll(
                    Column::with_children([text("sibling").into(), text("other").into()]),
                    axis,
                )
                .edge_fade(edge_fade()),
            )
            .into();
        let mut simulator = Simulator::with_size(Settings::default(), VISIBLE_SIZE, side_by_side);
        let sibling_before = simulator.find("sibling").expect("laid out").bounds();
        wheel_down(&mut simulator, ScrollDelta::Lines { x: 0.0, y: -2.0 });
        assert_eq!(
            simulator.find("sibling").expect("laid out").bounds(),
            sibling_before
        );
    }

    #[test]
    fn a_selected_child_is_scrolled_clear_of_the_fades() {
        let axis = Axis::Horizontal;
        let selected_position = ENTRY_COUNT / 2;
        let mut simulator = Simulator::with_size(
            Settings::default(),
            VISIBLE_SIZE,
            scroller_over(axis).selected(selected_position),
        );
        let now = Instant::now();
        let _redrawn = simulator.simulate([
            Event::Window(window::Event::RedrawRequested(now)),
            Event::Window(window::Event::RedrawRequested(now + Duration::from_secs(1))),
        ]);
        let selected = simulator
            .find(entry_label(selected_position).as_str())
            .expect("the selected entry is laid out")
            .bounds();
        assert!(selected.x >= FADE_EXTENT_PX - 1.0, "{selected:?}");
        assert!(
            selected.x + selected.width <= VISIBLE_SIZE.width - FADE_EXTENT_PX + 1.0,
            "{selected:?}"
        );
    }

    #[test]
    fn a_selected_span_inside_the_clear_area_does_not_scroll() {
        let offset_for = |start, end, offset| {
            SelectedSpan { start, end }.offset_clear_of_fades(offset, 300.0, 50.0)
        };
        let close_to = |first: f32, second: f32| (first - second).abs() < f32::EPSILON;
        assert!(close_to(offset_for(120.0, 180.0, 0.0), 0.0));
        assert!(close_to(offset_for(120.0, 180.0, 100.0), 70.0));
        assert!(close_to(offset_for(240.0, 290.0, 0.0), 40.0));
    }

    #[test]
    fn replaced_content_resets_the_scroll_and_jumps_to_its_selection() {
        let first: Element<'_, ()> = scroller_over(Axis::Horizontal).showing(1).into();
        let mut tree = Tree::new(&first);
        {
            let state = tree.state.downcast_mut::<ScrollerState>();
            state.scroll_motion.jump_to(120.0);
            state.followed_selection = FollowedSelection::FollowedAt(3);
        }
        let same: Element<'_, ()> = scroller_over(Axis::Horizontal).showing(1).into();
        tree.diff(&same);
        let state = tree.state.downcast_ref::<ScrollerState>();
        assert!((state.scroll_motion.drawn_offset - 120.0).abs() < f32::EPSILON);
        let replaced: Element<'_, ()> = scroller_over(Axis::Horizontal).showing(2).into();
        tree.diff(&replaced);
        let state = tree.state.downcast_ref::<ScrollerState>();
        assert!(state.scroll_motion.drawn_offset.abs() < f32::EPSILON);
        assert_eq!(state.followed_selection, FollowedSelection::AwaitingJump);
    }

    #[test]
    fn a_held_button_sees_the_real_pointer_while_content_slides() {
        let scroller: Element<'_, ()> = scroller_over(Axis::Horizontal)
            .paced(ScrollPacing::Eased(Duration::from_millis(160)))
            .into();
        let mut tree = Tree::new(&scroller);
        let state = tree.state.downcast_mut::<ScrollerState>();
        let pointer = Cursor::Available(Point::new(50.0, 5.0));
        let start = Instant::now();
        let frame = |now| super::ScrollFrame {
            now,
            max_offset: 1000.0,
            pointer_hover: PointerHover::Over,
        };
        state.scroll_motion.set_target(100.0);
        let _started = state.scroll_motion.advance(frame(start));
        let _midway = state
            .scroll_motion
            .advance(frame(start + Duration::from_millis(80)));
        assert_ne!(state.content_cursor(Axis::Horizontal, pointer), pointer);
        state.pointer_button = PointerButton::Held;
        assert_eq!(state.content_cursor(Axis::Horizontal, pointer), pointer);
    }
}
