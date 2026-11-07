use chrono::{Weekday, WeekdaySet};
use corredo::{
    context_menu::{
        ContextMenuLine, ContextMenuRow, ContextMenuRowPress, ContextMenuTrailing, context_menu,
    },
    fill_gauge::FillGauge,
    inline_slider::inline_slider,
    popup_trigger::{PopupVisibility, popup_trigger},
    pressable_text::pressable_text,
    selection_scroller::{ScrollAxis, selection_scroller},
    theme_colors::primary_color,
    toggle_switch::{TogglePosition, toggle_switch},
    transitioning_text::animated_text,
    weekday_picker::weekday_picker,
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
    Slid(f32),
    DayToggled(Weekday),
}

const WINDOW_SIZE: Size = Size::new(600.0, 400.0);

fn plain_context_menu<'a>() -> Element<'a, GalleryMessage> {
    context_menu([ContextMenuLine::Row(ContextMenuRow {
        leading: None,
        label: Some("menu entry".to_owned()),
        trailing: ContextMenuTrailing::Submenu { open: false },
        depth: 0,
        press: ContextMenuRowPress::Enabled(GalleryMessage::Pressed),
    })])
    .into()
}

fn gallery<'a>() -> Element<'a, GalleryMessage> {
    column![
        animated_text("animated label"),
        pressable_text(text("pressable label"), (), |()| GalleryMessage::Pressed),
        inline_slider(0.0..=10.0, 5.0, GalleryMessage::Slid),
        toggle_switch(TogglePosition::On),
        FillGauge::new(0.5, primary_color, 16.0),
        weekday_picker(WeekdaySet::EMPTY, GalleryMessage::DayToggled),
        plain_context_menu(),
        selection_scroller(text("scrolled label"), ScrollAxis::Vertical),
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
    let mut simulator = Simulator::with_size(Settings::default(), WINDOW_SIZE, gallery());
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
    let mut simulator =
        Simulator::with_size(Settings::default(), WINDOW_SIZE, plain_context_menu());
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
