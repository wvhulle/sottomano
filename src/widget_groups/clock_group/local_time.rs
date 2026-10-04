use chrono::{DateTime, Local, Locale, Utc};

use super::clock_reading::ClockReading;
use crate::{
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, row},
    },
};

pub struct LocalTime(pub DateTime<Local>);

impl LocalTime {
    pub fn reading(&self, locale: Locale) -> ClockReading {
        ClockReading {
            icon: Phosphor::Clock(Weight::Regular).into(),
            label: TimePart::TimeOfDay.format(&self.0, locale),
            target: HoverTarget::LocalTime,
        }
    }

    pub fn tooltip(&self, locale: Locale) -> Tooltip {
        Tooltip::table(vec![
            row("UTC offset", self.0.format("%:z").to_string()),
            row(
                "UTC",
                TimePart::TimeOfDay.format_in_own_zone(&self.0.with_timezone(&Utc), locale),
            ),
            row("Unix time", self.0.timestamp().to_string()),
        ])
    }
}
