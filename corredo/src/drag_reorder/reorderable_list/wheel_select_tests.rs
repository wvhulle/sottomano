use iced::{
    Color, Element, Event, Point, Settings, Size,
    mouse::{Event as MouseEvent, ScrollDelta},
    widget::text,
};
use iced_test::Simulator;

use super::reorderable_list;
use crate::orientation::Edge;

const SLOT_KEYS: [u8; 3] = [10, 20, 30];
const WHEEL_UP: ScrollDelta = ScrollDelta::Lines { x: 0.0, y: 1.0 };
const WHEEL_DOWN: ScrollDelta = ScrollDelta::Lines { x: 0.0, y: -1.0 };

fn selected_after_wheel(highlighted: u8, delta: ScrollDelta) -> Vec<u8> {
    let slots = SLOT_KEYS
        .into_iter()
        .map(|key| -> (u8, Element<'_, u8>) { (key, text(format!("slot {key}")).into()) });
    let list: Element<'_, u8> = reorderable_list(slots, Edge::Left)
        .highlight(highlighted, |_| Color::WHITE)
        .on_wheel_select(|key| key)
        .into();
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(200.0, 200.0), list);
    let pointer = simulator
        .find("slot 20")
        .expect("the middle slot is laid out")
        .bounds()
        .center();
    simulator.point_at(pointer);
    let _moved = simulator.simulate([Event::Mouse(MouseEvent::CursorMoved {
        position: Point::new(pointer.x, pointer.y),
    })]);
    let _wheeled = simulator.simulate([Event::Mouse(MouseEvent::WheelScrolled { delta })]);
    simulator.into_messages().collect()
}

#[test]
fn the_wheel_selects_the_neighbour_of_the_highlighted_slot() {
    assert_eq!(selected_after_wheel(20, WHEEL_UP), [10]);
    assert_eq!(selected_after_wheel(20, WHEEL_DOWN), [30]);
}

#[test]
fn the_wheel_stops_at_either_end() {
    assert!(selected_after_wheel(10, WHEEL_UP).is_empty());
    assert!(selected_after_wheel(30, WHEEL_DOWN).is_empty());
}
