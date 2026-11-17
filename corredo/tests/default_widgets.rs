use chrono::{Weekday, WeekdaySet};
use corredo::{
    changing_text::animated_text,
    gauges::FillGauge,
    orientation::Axis,
    popovers::{
        MenuItem, MenuRow, MenuRowContent, NestedMenuAction, NestedMenuPresentation,
        NestedMenuState, PopupVisibility, nested_menu, popup_trigger,
    },
    press_actions::flash_on_press_text,
    selection_scroll::selection_following_scroll,
    theming::primary_color,
    value_controls::{TogglePosition, inline_slider, toggle_switch, weekday_picker},
};
use iced::{
    Element, Point, Settings, Size, Theme,
    mouse::{Button, Event as MouseEvent},
    widget::{column, text},
};
use iced_test::Simulator;

#[derive(Debug, Clone, PartialEq)]
enum GalleryMessage {
    Pressed,
    MenuChanged(NestedMenuAction<u32>),
    Slid(f32),
    DayToggled(Weekday),
}

const WINDOW_SIZE: Size = Size::new(600.0, 400.0);

fn plain_context_menu(menu_state: &NestedMenuState<u32>) -> Element<'static, GalleryMessage> {
    nested_menu(
        [MenuItem::new(
            MenuRow::new(MenuRowContent::new("menu entry")).on_press(GalleryMessage::Pressed),
        )
        .into()],
        menu_state,
        NestedMenuPresentation::default(),
        GalleryMessage::MenuChanged,
    )
    .into()
}

fn gallery(menu_state: &NestedMenuState<u32>) -> Element<'static, GalleryMessage> {
    column![
        animated_text("animated label"),
        flash_on_press_text(text("pressable label"), (), |()| GalleryMessage::Pressed),
        inline_slider(0.0..=10.0, 5.0, GalleryMessage::Slid),
        toggle_switch(TogglePosition::On),
        FillGauge::new(0.5, primary_color, 16.0),
        weekday_picker(WeekdaySet::EMPTY, GalleryMessage::DayToggled),
        plain_context_menu(menu_state),
        selection_following_scroll(text("scrolled label"), Axis::Vertical),
        popup_trigger(
            text("trigger label"),
            text("popup label"),
            PopupVisibility::Open,
        ),
    ]
    .into()
}

#[test]
fn every_widget_lays_out_and_renders_with_only_its_essential_arguments() {
    let menu_state = NestedMenuState::default();
    let mut simulator =
        Simulator::with_size(Settings::default(), WINDOW_SIZE, gallery(&menu_state));
    for label in [
        "animated label",
        "pressable label",
        "menu entry",
        "scrolled label",
        "trigger label",
        "popup label",
    ] {
        assert!(simulator.find(label).is_ok(), "{label} is missing");
    }
    let _rendered = simulator.snapshot(&Theme::Dark).expect("the frame renders");
}

#[test]
fn a_default_context_menu_entry_publishes_its_message() {
    let menu_state = NestedMenuState::default();
    let mut simulator = Simulator::with_size(
        Settings::default(),
        WINDOW_SIZE,
        plain_context_menu(&menu_state),
    );
    let entry: Point = simulator
        .find("menu entry")
        .expect("the entry is on screen")
        .visible_bounds()
        .expect("the entry is visible")
        .center();
    simulator.point_at(entry);
    let _pressed = simulator.simulate([
        iced::Event::Mouse(MouseEvent::CursorMoved { position: entry }),
        iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left)),
        iced::Event::Mouse(MouseEvent::ButtonReleased(Button::Left)),
    ]);
    assert_eq!(
        simulator.into_messages().collect::<Vec<_>>(),
        [GalleryMessage::Pressed]
    );
}
