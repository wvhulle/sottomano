use chrono::{Locale, NaiveDate, TimeDelta, Weekday, WeekdaySet};
use iced::{
    Alignment, Element,
    widget::{Row, button},
};
use sottomano_config::Config;

use crate::{
    hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
    widgets::{
        flat_button,
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        source_reading::icon_value_gap,
        text::{FontFaces, panel_text},
        toggle_control::SwitchPresses,
    },
};

const A_MONDAY: NaiveDate = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");

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
    pub on_tooltip: fn(TooltipMessage) -> Message,
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

    fn day<'a>(&self, day: Weekday) -> Element<'a, Message>
    where
        Message: 'a,
    {
        let chosen = self.chosen.contains(day);
        let tint = ReadingStateTint {
            state: ReadingDisplayState::of_toggle(chosen),
            opacity: self.reveal,
        };
        let faces = FontFaces::from(&self.config.theme.font);
        let initial = panel_text(
            &self.config.theme.font,
            weekday_initial(day, self.config.calendar.locale),
        )
        .font(if chosen { faces.bold } else { faces.regular })
        .style(tint.text_style());
        hoverable(
            button(initial)
                .padding(0)
                .style(flat_button(move |theme| tint.color(theme)))
                .on_press((self.messages.on_toggle)(day))
                .into(),
            (self.messages.target)(day),
            PressHints {
                left: Some(if chosen {
                    self.presses.turning_off
                } else {
                    self.presses.turning_on
                }),
                ..PressHints::default()
            },
            self.messages.on_tooltip,
        )
    }
}

impl<'a, Message: 'a + Clone> From<WeekdayPicker<'_, Message>> for Element<'a, Message> {
    fn from(picker: WeekdayPicker<'_, Message>) -> Self {
        Row::with_children(
            WeekdaySet::ALL
                .iter(Weekday::Mon)
                .map(|day| picker.day(day)),
        )
        .spacing(icon_value_gap(picker.config))
        .align_y(Alignment::Center)
        .into()
    }
}

#[must_use]
pub fn weekday_name(day: Weekday, locale: Locale) -> String {
    a_date_on(day).format_localized("%A", locale).to_string()
}

fn weekday_initial(day: Weekday, locale: Locale) -> String {
    a_date_on(day)
        .format_localized("%a", locale)
        .to_string()
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .collect()
}

fn a_date_on(day: Weekday) -> NaiveDate {
    A_MONDAY + TimeDelta::days(i64::from(day.num_days_from_monday()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_follow_the_locale() {
        assert_eq!(weekday_initial(Weekday::Wed, Locale::en_US), "W");
        assert_eq!(weekday_initial(Weekday::Wed, Locale::nl_BE), "W");
        assert_eq!(weekday_initial(Weekday::Thu, Locale::nl_BE), "D");
    }

    #[test]
    fn names_follow_the_locale() {
        assert_eq!(weekday_name(Weekday::Mon, Locale::en_US), "Monday");
        assert_eq!(weekday_name(Weekday::Mon, Locale::nl_BE), "maandag");
    }
}
