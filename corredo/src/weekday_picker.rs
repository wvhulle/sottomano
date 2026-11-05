use chrono::{Locale, NaiveDate, TimeDelta, Weekday, WeekdaySet};
use iced::{
    Alignment, Color, Element, Theme,
    widget::{Row, button, text},
};

use crate::{
    flat_button::flat_button,
    font_faces::{FontFaces, TextFace},
};

const A_MONDAY: NaiveDate = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");

pub struct WeekdayPickerStyle {
    pub face: TextFace,
    pub faces: FontFaces,
    pub spacing_px: f32,
    pub locale: Locale,
}

pub fn weekday_picker<'a, Message: Clone + 'a>(
    chosen: WeekdaySet,
    style: &WeekdayPickerStyle,
    day_color: impl Fn(bool, &Theme) -> Color + Copy + 'a,
    on_toggle: impl Fn(Weekday) -> Message,
    wrap_day: impl Fn(Weekday, bool, Element<'a, Message>) -> Element<'a, Message>,
) -> Element<'a, Message> {
    Row::with_children(WeekdaySet::ALL.iter(Weekday::Mon).map(|day| {
        let is_chosen = chosen.contains(day);
        let initial = style
            .face
            .text(weekday_initial(day, style.locale))
            .font(if is_chosen {
                style.faces.bold
            } else {
                style.faces.regular
            })
            .style(move |theme| text::Style {
                color: Some(day_color(is_chosen, theme)),
            });
        wrap_day(
            day,
            is_chosen,
            button(initial)
                .padding(0)
                .style(flat_button(move |theme| day_color(is_chosen, theme)))
                .on_press(on_toggle(day))
                .into(),
        )
    }))
    .spacing(style.spacing_px)
    .align_y(Alignment::Center)
    .into()
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
