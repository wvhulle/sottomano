use chrono::{Weekday, WeekdaySet};
use corredo::{
    changing_text::animated_text,
    data_tables::text_grouped_table,
    gauges::FillGauge,
    popovers::{
        MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuSubmenu, NestedMenuAction,
        NestedMenuPresentation, NestedMenuState, PopupVisibility, nested_menu, popup_trigger,
    },
    press_actions::flash_on_press_text,
    theming::primary_color,
    value_controls::{TogglePosition, inline_slider, toggle_switch, weekday_picker},
};
use iced::{
    Element,
    widget::{button, column, mouse_area, text},
};

#[derive(Debug, Clone)]
enum GalleryMessage {
    Slid(f32),
    SwitchToggled,
    DayToggled(Weekday),
    PopupToggled,
    PopupDismissed,
    MenuEntryPressed,
    MenuChanged(NestedMenuAction<&'static str>),
    TextPressed,
}

#[derive(Debug)]
struct WidgetGallery {
    slider_value: f32,
    toggle_position: TogglePosition,
    chosen_days: WeekdaySet,
    popup_visibility: PopupVisibility,
    menu_presses: u32,
    menu_state: NestedMenuState<&'static str>,
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
            menu_state: NestedMenuState::default(),
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
            GalleryMessage::MenuEntryPressed => {
                self.menu_presses += 1;
                self.menu_state.close();
            }
            GalleryMessage::MenuChanged(action) => self.menu_state.perform(action),
            GalleryMessage::TextPressed => self.text_presses += 1,
        }
    }

    fn view(&self) -> Element<'_, GalleryMessage> {
        let table = text_grouped_table(
            ["Moons"],
            [
                ("Earth".to_owned(), vec!["1".to_owned()]),
                ("Mars".to_owned(), vec!["2".to_owned()]),
            ],
        );
        let moon_item = |moon: &'static str| -> MenuEntry<'_, &'static str, GalleryMessage> {
            MenuItem::new(
                MenuRow::new(MenuRowContent::new(moon)).on_press(GalleryMessage::MenuEntryPressed),
            )
            .into()
        };
        let menu = nested_menu(
            [
                MenuItem::new(
                    MenuRow::new(MenuRowContent::new(format!(
                        "Pressed {} times",
                        self.menu_presses
                    )))
                    .on_press(GalleryMessage::MenuEntryPressed),
                )
                .into(),
                MenuEntry::Divider,
                MenuSubmenu::new(
                    "planets",
                    MenuRowContent::new("Planets"),
                    [
                        moon_item("Earth"),
                        MenuSubmenu::new(
                            "jupiter",
                            MenuRowContent::new("Jupiter"),
                            [moon_item("Io"), moon_item("Europa")],
                        )
                        .into(),
                    ],
                )
                .into(),
            ],
            &self.menu_state,
            NestedMenuPresentation::default(),
            GalleryMessage::MenuChanged,
        );
        column![
            animated_text(format!("Slider at {:.0}", self.slider_value))
                .identified_by(&"slider label"),
            inline_slider(0.0..=10.0, self.slider_value, GalleryMessage::Slid),
            mouse_area(toggle_switch(self.toggle_position)).on_press(GalleryMessage::SwitchToggled),
            FillGauge::new(self.slider_value / 10.0, primary_color, 32.0),
            weekday_picker(self.chosen_days, GalleryMessage::DayToggled),
            flash_on_press_text(
                text(format!("Pressed {} times", self.text_presses)),
                (),
                |()| GalleryMessage::TextPressed
            ),
            Element::from(menu),
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
