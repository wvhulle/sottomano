use std::fmt::Display;

use chrono::{DateTime, Local, Locale, TimeZone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalizedTimeFormat {
    TimeOfDay,
    Date,
    DateAndTime,
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
        at.format_localized(self.specifier(), locale).to_string()
    }

    fn specifier(self) -> &'static str {
        match self {
            LocalizedTimeFormat::TimeOfDay => "%X",
            LocalizedTimeFormat::Date => "%x",
            LocalizedTimeFormat::DateAndTime => "%x %X",
            LocalizedTimeFormat::Weekday => "%A",
        }
    }
}
