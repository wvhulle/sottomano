use chrono::{DateTime, Datelike, Local, Locale, Utc};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::TimePart,
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, TooltipBody},
};

pub struct LocalDate(pub DateTime<Local>);

impl BarWidget for LocalDate {
    fn name(&self) -> &'static str {
        "Local date"
    }

    fn icon(&self) -> Glyph {
        Phosphor::CalendarBlank(Weight::Regular).into()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        Some(TimePart::Date.format(&self.0, locale))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new(
                "Weekday",
                Cell::Time {
                    at: self.0.with_timezone(&Utc),
                    part: TimePart::Weekday,
                },
            ),
            KeyValue::new("Week", self.0.iso_week().week().to_string()),
            KeyValue::new("Day of year", self.0.ordinal().to_string()),
        ])
    }
}
