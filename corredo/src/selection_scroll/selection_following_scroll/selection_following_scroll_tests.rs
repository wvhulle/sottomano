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
    ScrollPacing, SelectionFollowingScroll, WheelAxes,
    scroll_motion::ScrollFrame,
    scroll_offset_steering::{FollowedSelection, PointerButton, ScrollerState, SelectedSpan},
    selection_following_scroll,
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
    let offset_for =
        |start, end, offset| SelectedSpan { start, end }.offset_clear_of_fades(offset, 300.0, 50.0);
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
    let frame = |now| ScrollFrame {
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
