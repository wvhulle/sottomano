use chrono::{DateTime, Locale, TimeDelta, Utc};
use corredo::gauges::TimelineTextFormat;

use crate::widgets::{localized_durations::duration_text, localized_time::LocalizedTimeFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalizedTimelineText {
    pub locale: Locale,
}

impl TimelineTextFormat for LocalizedTimelineText {
    fn clock_time_text(&self, at: DateTime<Utc>) -> String {
        LocalizedTimeFormat::HourAndMinute.format(&at, self.locale)
    }

    fn length_text(&self, length: TimeDelta) -> String {
        let shown_length = if length >= TimeDelta::minutes(1) {
            TimeDelta::minutes(length.num_minutes())
        } else {
            length
        };
        duration_text(shown_length, self.locale)
    }
}
