use chrono::{DateTime, Local, Locale};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::TimePart,
    phosphor::{Phosphor, Weight},
    tooltip_body::{KeyValue, Section, TooltipBody},
};

pub struct LocalTime(pub DateTime<Local>);

impl BarWidget for LocalTime {
    fn name(&self) -> &'static str {
        "Local time"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        Some(TimePart::TimeOfDay.format(&self.0, locale))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled_dictionary([
            KeyValue::new("UTC offset", self.0.format("%:z").to_string()),
            KeyValue::new("Unix time", self.0.timestamp().to_string()),
        ])])
    }
}
