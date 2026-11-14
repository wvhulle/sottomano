use chrono::{Weekday, WeekdaySet};
use corredo::value_controls::{DaySelection, weekday_picker as themed_weekday_picker};
use iced::Element;
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        toggle_control::SwitchPresses,
    },
    hover_cards::{HoverCardMessage, HoverTarget, PressHints, hoverable},
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
    presses: SwitchPresses,
    reveal: f32,
    messages: WeekdayPickerMessages<Message>,
}

pub struct WeekdayPickerMessages<Message> {
    pub target: fn(Weekday) -> HoverTarget,
    pub on_toggle: fn(Weekday) -> Message,
    pub on_hover_card: fn(HoverCardMessage) -> Message,
}

impl<'c, Message: Clone> WeekdayPicker<'c, Message> {
    #[must_use]
    pub fn new(
        config: &'c Config,
        chosen: WeekdaySet,
        presses: SwitchPresses,
        reveal: f32,
        messages: WeekdayPickerMessages<Message>,
    ) -> Self {
        Self {
            config,
            chosen,
            presses,
            reveal,
            messages,
        }
    }
}

impl<'a, Message: 'a + Clone> From<WeekdayPicker<'_, Message>> for Element<'a, Message> {
    fn from(picker: WeekdayPicker<'_, Message>) -> Self {
        let font = &picker.config.theme.font;
        let reveal = picker.reveal;
        let presses = picker.presses;
        let messages = picker.messages;
        themed_weekday_picker(picker.chosen, messages.on_toggle)
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
            .wrap_day(move |day, day_selection, day_button| {
                hoverable(
                    day_button,
                    (messages.target)(day),
                    PressHints {
                        left: Some(match day_selection {
                            DaySelection::Chosen => presses.turning_off,
                            DaySelection::Unchosen => presses.turning_on,
                        }),
                        ..PressHints::default()
                    },
                    messages.on_hover_card,
                )
            })
            .into()
    }
}
