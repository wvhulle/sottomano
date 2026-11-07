use chrono::{Weekday, WeekdaySet};
use corredo::{
    context_menu::{
        ContextMenuLine, ContextMenuRow, ContextMenuRowPress, ContextMenuTrailing, context_menu,
    },
    fill_gauge::FillGauge,
    inline_slider::inline_slider,
    popup_trigger::{PopupVisibility, popup_trigger},
    pressable_text::pressable_text,
    record_table::text_record_table,
    theme_colors::primary_color,
    toggle_switch::{TogglePosition, toggle_switch},
    transitioning_text::animated_text,
    weekday_picker::weekday_picker,
};
use iced::{
    Element,
    widget::{button, column, mouse_area, text},
};

#[derive(Debug, Clone, Copy)]
enum GalleryMessage {
    Slid(f32),
    SwitchToggled,
    DayToggled(Weekday),
    PopupToggled,
    PopupDismissed,
    MenuEntryPressed,
    TextPressed,
}

#[derive(Debug)]
struct WidgetGallery {
    slider_value: f32,
    toggle_position: TogglePosition,
    chosen_days: WeekdaySet,
    popup_visibility: PopupVisibility,
    menu_presses: u32,
    text_presses: u32,
}

impl Default for WidgetGallery {
    fn default() -> Self {
        Self {
            slider_value: 5.0,
            toggle_position: TogglePosition::On,
            chosen_days: WeekdaySet::EMPTY,
            popup_visibility: PopupVisibility::Closed,
            menu_presses: 0,
            text_presses: 0,
        }
    }
}

impl WidgetGallery {
    fn update(&mut self, message: GalleryMessage) {
        match message {
            GalleryMessage::Slid(value) => self.slider_value = value,
            GalleryMessage::SwitchToggled => {
                self.toggle_position = match self.toggle_position {
                    TogglePosition::On => TogglePosition::Off,
                    TogglePosition::Off => TogglePosition::On,
                };
            }
            GalleryMessage::DayToggled(day) => {
                self.chosen_days = if self.chosen_days.contains(day) {
                    self.chosen_days.difference(WeekdaySet::single(day))
                } else {
                    self.chosen_days.union(WeekdaySet::single(day))
                };
            }
            GalleryMessage::PopupToggled => {
                self.popup_visibility = match self.popup_visibility {
                    PopupVisibility::Open => PopupVisibility::Closed,
                    PopupVisibility::Closed => PopupVisibility::Open,
                };
            }
            GalleryMessage::PopupDismissed => self.popup_visibility = PopupVisibility::Closed,
            GalleryMessage::MenuEntryPressed => self.menu_presses += 1,
            GalleryMessage::TextPressed => self.text_presses += 1,
        }
    }

    fn view(&self) -> Element<'_, GalleryMessage> {
        let table = text_record_table(
            ["Moons"],
            [
                ("Earth".to_owned(), vec!["1".to_owned()]),
                ("Mars".to_owned(), vec!["2".to_owned()]),
            ],
        );
        let menu = context_menu([ContextMenuLine::Row(ContextMenuRow {
            leading: None,
            label: Some(format!("Pressed {} times", self.menu_presses)),
            trailing: ContextMenuTrailing::Nothing,
            depth: 0,
            press: ContextMenuRowPress::Enabled(GalleryMessage::MenuEntryPressed),
        })]);
        column![
            animated_text(format!("Slider at {:.0}", self.slider_value))
                .identified_by(&"slider label"),
            inline_slider(0.0..=10.0, self.slider_value, GalleryMessage::Slid),
            mouse_area(toggle_switch(self.toggle_position)).on_press(GalleryMessage::SwitchToggled),
            FillGauge::new(self.slider_value / 10.0, primary_color, 32.0),
            weekday_picker(self.chosen_days, GalleryMessage::DayToggled),
            pressable_text(
                text(format!("Pressed {} times", self.text_presses)),
                (),
                |()| GalleryMessage::TextPressed
            ),
            menu,
            table,
            popup_trigger(
                button(text("Toggle popup")).on_press(GalleryMessage::PopupToggled),
                text("Anchored to the button"),
                self.popup_visibility,
            )
            .on_dismiss(GalleryMessage::PopupDismissed),
        ]
        .spacing(16)
        .padding(16)
        .into()
    }
}

fn main() -> iced::Result {
    iced::run(WidgetGallery::update, WidgetGallery::view)
}
