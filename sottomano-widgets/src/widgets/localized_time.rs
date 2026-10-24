use std::fmt::Display;

use chrono::{DateTime, Local, Locale, TimeZone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalizedTimeFormat {
    TimeOfDay,
    HourOfDay,
    HourAndMinute,
    Date,
    DateAndTime,
    DateAndMinute,
    Weekday,
}

impl LocalizedTimeFormat {
    pub fn format<Tz: TimeZone>(self, at: &DateTime<Tz>, locale: Locale) -> String
    where
        Tz::Offset: Display,
    {
        self.format_in_own_zone(&at.with_timezone(&Local), locale)
    }

    pub fn format_in_own_zone<Tz: TimeZone>(self, at: &DateTime<Tz>, locale: Locale) -> String
    where
        Tz::Offset: Display,
    {
        at.format_localized(self.specifier(at, locale), locale)
            .to_string()
    }

    fn specifier<Tz: TimeZone>(self, at: &DateTime<Tz>, locale: Locale) -> &'static str
    where
        Tz::Offset: Display,
    {
        match self {
            LocalizedTimeFormat::TimeOfDay => "%X",
            LocalizedTimeFormat::HourOfDay if uses_twelve_hour_clock(at, locale) => "%-I %p",
            LocalizedTimeFormat::HourOfDay => "%H",
            LocalizedTimeFormat::HourAndMinute if uses_twelve_hour_clock(at, locale) => "%-I:%M %p",
            LocalizedTimeFormat::HourAndMinute => "%H:%M",
            LocalizedTimeFormat::Date => "%x",
            LocalizedTimeFormat::DateAndTime => "%x %X",
            LocalizedTimeFormat::DateAndMinute if uses_twelve_hour_clock(at, locale) => {
                "%x %-I:%M %p"
            }
            LocalizedTimeFormat::DateAndMinute => "%x %H:%M",
            LocalizedTimeFormat::Weekday => "%A",
        }
    }
}

fn uses_twelve_hour_clock<Tz: TimeZone>(at: &DateTime<Tz>, locale: Locale) -> bool
where
    Tz::Offset: Display,
{
    !at.format_localized("%p", locale).to_string().is_empty()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn hour_of_day_omits_minutes_and_seconds() {
        let afternoon = "2026-10-19T14:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(
            LocalizedTimeFormat::HourOfDay.format_in_own_zone(&afternoon, Locale::en_US),
            "2 PM"
        );
        assert_eq!(
            LocalizedTimeFormat::HourOfDay.format_in_own_zone(&afternoon, Locale::nl_BE),
            "14"
        );
    }

    #[test]
    fn hour_and_minute_omits_seconds() {
        let afternoon = "2026-10-19T14:45:30Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(
            LocalizedTimeFormat::HourAndMinute.format_in_own_zone(&afternoon, Locale::en_US),
            "2:45 PM"
        );
        assert_eq!(
            LocalizedTimeFormat::HourAndMinute.format_in_own_zone(&afternoon, Locale::nl_BE),
            "14:45"
        );
    }
}
