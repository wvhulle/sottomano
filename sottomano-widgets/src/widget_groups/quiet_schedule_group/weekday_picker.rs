use chrono::{Locale, Weekday, WeekdaySet};
use corredo::value_controls::{
    DaySelection, weekday_name, weekday_picker as themed_weekday_picker,
};
use iced::Element;
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        toggle_control::SwitchPresses,
    },
    hover_cards::{MouseGesture, PressAction},
    widgets::text::{font_faces_of, text_face_of},
};

impl From<DaySelection> for ReadingDisplayState {
    fn from(day_selection: DaySelection) -> Self {
        match day_selection {
            DaySelection::Chosen => ReadingDisplayState::On,
            DaySelection::Unchosen => ReadingDisplayState::Off,
        }
    }
}

pub struct WeekdayPicker<'c, Message> {
    config: &'c Config,
    chosen: WeekdaySet,
    reveal: f32,
    messages: WeekdayPickerMessages<Message>,
}

pub struct WeekdayPickerMessages<Message> {
    pub on_toggle: fn(Weekday) -> Message,
}

#[must_use]
pub fn weekday_press_actions(
    chosen: WeekdaySet,
    presses: SwitchPresses,
    locale: Locale,
) -> Vec<PressAction> {
    WeekdaySet::ALL
        .iter(Weekday::Mon)
        .map(|day| {
            let press_does = if chosen.contains(day) {
                presses.turning_off
            } else {
                presses.turning_on
            };
            PressAction::new(
                MouseGesture::LeftClick,
                format!("{}: {press_does}", weekday_name(day, locale)),
            )
        })
        .collect()
}

impl<'c, Message: Clone> WeekdayPicker<'c, Message> {
    #[must_use]
    pub fn new(
        config: &'c Config,
        chosen: WeekdaySet,
        reveal: f32,
        messages: WeekdayPickerMessages<Message>,
    ) -> Self {
        Self {
            config,
            chosen,
            reveal,
            messages,
        }
    }
}

impl<'a, Message: 'a + Clone> From<WeekdayPicker<'_, Message>> for Element<'a, Message> {
    fn from(picker: WeekdayPicker<'_, Message>) -> Self {
        let font = &picker.config.theme.font;
        let reveal = picker.reveal;
        themed_weekday_picker(picker.chosen, picker.messages.on_toggle)
            .typeface(text_face_of(font), font_faces_of(font))
            .spacing(picker.config.theme.spacing.item_gap)
            .locale(picker.config.calendar.locale)
            .day_color(move |day_selection, theme| {
                ReadingStateTint {
                    state: ReadingDisplayState::from(day_selection),
                    opacity: reveal,
                }
                .color(theme)
            })
            .into()
    }
}
