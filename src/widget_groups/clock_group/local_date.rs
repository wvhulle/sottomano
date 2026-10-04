use chrono::{DateTime, Datelike, Local, Locale, Utc};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::TimePart,
    phosphor::{Phosphor, Weight},
    tooltip_body::{TooltipBody, row, time_row},
};

pub struct LocalDate(pub DateTime<Local>);

impl BarWidget for LocalDate {
    fn icon(&self) -> Glyph {
        Phosphor::CalendarBlank(Weight::Regular).into()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        Some(TimePart::Date.format(&self.0, locale))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            time_row("Weekday", self.0.with_timezone(&Utc), TimePart::Weekday),
            row("Week", self.0.iso_week().week().to_string()),
            row("Day of year", self.0.ordinal().to_string()),
        ])
    }
}
