use iced::{
    Element, Rectangle, Settings, Size,
    widget::{Column, text},
};
use iced_test::Simulator;

use super::{NotchRowAnchor, notched_panel};
use crate::popovers::{CascadeDirection, CascadeOrientation, PopupGrowth, cascade_row};

const ROW_OFFSET: f32 = 30.0;

fn cascade_with_card(
    direction: CascadeDirection,
    list_row_top: f32,
    card_label: &'static str,
) -> Element<'static, ()> {
    let orientation = CascadeOrientation {
        direction,
        growth: PopupGrowth::Downward,
    };
    let card = notched_panel(
        text(card_label),
        orientation,
        NotchRowAnchor::WithinParent {
            row_offset: ROW_OFFSET,
        },
    )
    .row_height(20.0);
    let list_rows = Column::with_children(
        ["list", "second", "third", "fourth", "fifth", "sixth"].map(|label| text(label).into()),
    );
    let list = notched_panel(
        list_rows,
        orientation,
        NotchRowAnchor::FromPrimaryTop(list_row_top),
    )
    .row_height(20.0)
    .beside([card]);
    cascade_row(text("primary").into(), [list.into()], orientation)
}

fn bounds_of(simulator: &mut Simulator<'_, ()>, label: &str) -> Rectangle {
    simulator
        .find(label)
        .unwrap_or_else(|_| panic!("{label} is laid out"))
        .bounds()
}

fn simulated(screen: Size, cascade: Element<'static, ()>) -> Simulator<'static, ()> {
    Simulator::with_size(Settings::default(), screen, cascade)
}

#[test]
fn a_nested_card_lines_up_with_its_row_and_has_room() {
    let mut simulator = simulated(
        Size::new(600.0, 400.0),
        cascade_with_card(CascadeDirection::Rightward, 40.0, "card"),
    );
    let list = bounds_of(&mut simulator, "list");
    let card = bounds_of(&mut simulator, "card");
    assert!(
        (card.y - list.y - ROW_OFFSET).abs() < 0.5,
        "list {list:?}, card {card:?}"
    );
    assert!(card.x > list.x + list.width, "list {list:?}, card {card:?}");
    assert!(card.width > 0.0, "card {card:?}");
}

#[test]
fn a_leftward_card_opens_left_of_its_list() {
    let mut simulator = simulated(
        Size::new(600.0, 400.0),
        cascade_with_card(CascadeDirection::Leftward, 40.0, "card"),
    );
    let list = bounds_of(&mut simulator, "list");
    let card = bounds_of(&mut simulator, "card");
    assert!(
        card.x + card.width <= list.x + 0.5,
        "list {list:?}, card {card:?}"
    );
    assert!(card.width > 0.0, "card {card:?}");
}

#[test]
fn a_nested_card_follows_a_list_pushed_back_on_screen() {
    let mut simulator = simulated(
        Size::new(600.0, 120.0),
        cascade_with_card(CascadeDirection::Rightward, 110.0, "card"),
    );
    let list = bounds_of(&mut simulator, "list");
    let card = bounds_of(&mut simulator, "card");
    assert!(
        list.y < 110.0,
        "the list is pushed back on screen, at {list:?}"
    );
    assert!(
        (card.y - list.y - ROW_OFFSET).abs() < 0.5,
        "list {list:?}, card {card:?}"
    );
}

#[test]
fn a_wider_card_leaves_the_list_width_unchanged() {
    let list_width = |card_label: &'static str| {
        let mut simulator = simulated(
            Size::new(600.0, 400.0),
            cascade_with_card(CascadeDirection::Leftward, 40.0, card_label),
        );
        bounds_of(&mut simulator, "sixth").width
    };
    assert!(
        (list_width("card") - list_width("a much much wider card than the list itself")).abs()
            < 0.5
    );
}
