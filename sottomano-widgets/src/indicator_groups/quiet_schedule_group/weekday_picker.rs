use chrono::{Locale, Weekday, WeekdaySet};
use corredo::value_controls::{
    DaySelection, weekday_name, weekday_picker as themed_weekday_picker,
};
use iced::Element;
use sottomano_toml_config::Config;

use crate::{
    group_indicators::{
        indicator_state::{IndicatorState, IndicatorStateTint},
        toggle_control::SwitchPresses,
    },
    hover_cards::{MouseGesture, PressAction},
    widgets::text::{resolved_font_faces, resolved_text_face},
};

impl From<DaySelection> for IndicatorState {
    fn from(day_selection: DaySelection) -> Self {
        match day_selection {
            DaySelection::Chosen => IndicatorState::On,
            DaySelection::Unchosen => IndicatorState::Off,
        }
    }
}

pub struct WeekdayPicker<'config, Message> {
    config: &'config Config,
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

impl<'config, Message: Clone> WeekdayPicker<'config, Message> {
    #[must_use]
    pub fn new(
        config: &'config Config,
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

impl<'view, Message: 'view + Clone> From<WeekdayPicker<'_, Message>> for Element<'view, Message> {
    fn from(picker: WeekdayPicker<'_, Message>) -> Self {
        let font = &picker.config.theme.font;
        let reveal = picker.reveal;
        themed_weekday_picker(picker.chosen, picker.messages.on_toggle)
            .typeface(resolved_text_face(font), resolved_font_faces(font))
            .spacing(picker.config.theme.spacing.item_gap)
            .locale(picker.config.calendar.locale)
            .day_color(move |day_selection, theme| {
                IndicatorStateTint {
                    state: IndicatorState::from(day_selection),
                    opacity: reveal,
                }
                .color(theme)
            })
            .into()
    }
}
