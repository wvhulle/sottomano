use std::rc::Rc;

use chrono::{Locale, NaiveDate, TimeDelta, Weekday, WeekdaySet};
use iced::{
    Alignment, Color, Element, Theme,
    font::Family,
    widget::{Row, button, text},
};

use crate::{
    press_actions::flat_button,
    theming::{FontFaces, TextFace, primary_color, secondary_text_color},
};

const A_MONDAY: NaiveDate = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");

const DEFAULT_SPACING_PX: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaySelection {
    Chosen,
    Unchosen,
}

type DayColor<'a> = Rc<dyn Fn(DaySelection, &Theme) -> Color + 'a>;
type DayWrapper<'a, Message> =
    Box<dyn Fn(Weekday, DaySelection, Element<'a, Message>) -> Element<'a, Message> + 'a>;

pub struct WeekdayPicker<'a, Message> {
    chosen: WeekdaySet,
    on_toggle: Box<dyn Fn(Weekday) -> Message + 'a>,
    face: TextFace,
    faces: FontFaces,
    spacing_px: f32,
    locale: Locale,
    day_color: DayColor<'a>,
    wrap_day: DayWrapper<'a, Message>,
}

pub fn weekday_picker<'a, Message: Clone + 'a>(
    chosen: WeekdaySet,
    on_toggle: impl Fn(Weekday) -> Message + 'a,
) -> WeekdayPicker<'a, Message> {
    WeekdayPicker {
        chosen,
        on_toggle: Box::new(on_toggle),
        face: TextFace::default(),
        faces: FontFaces::from(Family::SansSerif),
        spacing_px: DEFAULT_SPACING_PX,
        locale: Locale::POSIX,
        day_color: Rc::new(|day_selection, theme| match day_selection {
            DaySelection::Chosen => primary_color(theme),
            DaySelection::Unchosen => secondary_text_color(theme),
        }),
        wrap_day: Box::new(|_, _, day_button| day_button),
    }
}

impl<'a, Message> WeekdayPicker<'a, Message> {
    #[must_use]
    pub fn typeface(self, face: TextFace, faces: FontFaces) -> Self {
        Self {
            face,
            faces,
            ..self
        }
    }

    #[must_use]
    pub fn spacing(self, spacing_px: f32) -> Self {
        Self { spacing_px, ..self }
    }

    #[must_use]
    pub fn locale(self, locale: Locale) -> Self {
        Self { locale, ..self }
    }

    #[must_use]
    pub fn day_color(self, day_color: impl Fn(DaySelection, &Theme) -> Color + 'a) -> Self {
        Self {
            day_color: Rc::new(day_color),
            ..self
        }
    }

    #[must_use]
    pub fn wrap_day(
        self,
        wrap_day: impl Fn(Weekday, DaySelection, Element<'a, Message>) -> Element<'a, Message> + 'a,
    ) -> Self {
        Self {
            wrap_day: Box::new(wrap_day),
            ..self
        }
    }
}

impl<'a, Message: Clone + 'a> From<WeekdayPicker<'a, Message>> for Element<'a, Message> {
    fn from(picker: WeekdayPicker<'a, Message>) -> Self {
        Row::with_children(WeekdaySet::ALL.iter(Weekday::Mon).map(|day| {
            let day_selection = if picker.chosen.contains(day) {
                DaySelection::Chosen
            } else {
                DaySelection::Unchosen
            };
            let text_color = Rc::clone(&picker.day_color);
            let button_color = Rc::clone(&picker.day_color);
            let initial = picker
                .face
                .text(weekday_initial(day, picker.locale))
                .font(match day_selection {
                    DaySelection::Chosen => picker.faces.bold,
                    DaySelection::Unchosen => picker.faces.regular,
                })
                .style(move |theme| text::Style {
                    color: Some(text_color(day_selection, theme)),
                });
            (picker.wrap_day)(
                day,
                day_selection,
                button(initial)
                    .padding(0)
                    .style(flat_button(move |theme| button_color(day_selection, theme)))
                    .on_press((picker.on_toggle)(day))
                    .into(),
            )
        }))
        .spacing(picker.spacing_px)
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
