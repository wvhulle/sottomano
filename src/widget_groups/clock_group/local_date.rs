use chrono::{DateTime, Datelike, Local, Locale};

use super::clock_reading::ClockReading;
use crate::{
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, row},
    },
};

pub struct LocalDate(pub DateTime<Local>);

impl LocalDate {
    pub fn reading(&self, locale: Locale) -> ClockReading {
        ClockReading {
            icon: Phosphor::CalendarBlank(Weight::Regular).into(),
            label: TimePart::Date.format(&self.0, locale),
            target: HoverTarget::LocalDate,
        }
    }

    pub fn tooltip(&self, locale: Locale) -> Tooltip {
        Tooltip::table(vec![
            row("Weekday", self.0.format_localized("%A", locale).to_string()),
            row("Week", self.0.iso_week().week().to_string()),
            row("Day of year", self.0.ordinal().to_string()),
        ])
    }
}
