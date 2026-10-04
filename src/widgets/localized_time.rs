use std::fmt::Display;

use chrono::{DateTime, Local, Locale, TimeZone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePart {
    TimeOfDay,
    Date,
    DateAndTime,
}

impl TimePart {
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
            TimePart::TimeOfDay => "%X",
            TimePart::Date => "%x",
            TimePart::DateAndTime => "%x %X",
        }
    }
}
