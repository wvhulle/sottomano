use chrono::{DateTime, Local, Locale, Offset};
use chrono_tz::Tz;

use super::clock_reading::ClockReading;
use crate::{
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, row},
    },
};

pub struct ZoneTime(pub DateTime<Tz>);

impl ZoneTime {
    pub fn reading(&self, locale: Locale) -> ClockReading {
        ClockReading {
            icon: Phosphor::GlobeHemisphereWest(Weight::Regular).into(),
            label: format!(
                "{} {}",
                city_name(self.0.timezone()),
                TimePart::TimeOfDay.format_in_own_zone(&self.0, locale)
            ),
            target: HoverTarget::ZoneTime(self.0.timezone()),
        }
    }

    pub fn tooltip(&self, locale: Locale) -> Tooltip {
        Tooltip::table(vec![
            row("Zone", self.0.timezone().name()),
            row("Abbreviation", self.0.format("%Z").to_string()),
            row("UTC offset", self.0.format("%:z").to_string()),
            row("Date", TimePart::Date.format_in_own_zone(&self.0, locale)),
            row(
                "Offset from local",
                signed_hours_and_minutes(self.seconds_ahead_of_local()),
            ),
        ])
    }

    fn seconds_ahead_of_local(&self) -> i32 {
        let local_offset = self.0.with_timezone(&Local).offset().local_minus_utc();
        self.0.offset().fix().local_minus_utc() - local_offset
    }
}

fn city_name(zone: Tz) -> String {
    zone.name()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .replace('_', " ")
}

fn signed_hours_and_minutes(seconds: i32) -> String {
    let sign = if seconds < 0 { '−' } else { '+' };
    let minutes = seconds.unsigned_abs() / 60;
    let hours = minutes / 60;
    match minutes % 60 {
        0 => format!("{sign}{hours} h"),
        remaining => format!("{sign}{hours} h {remaining} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_name_is_the_last_zone_segment_with_spaces() {
        assert_eq!(
            city_name(Tz::America__Argentina__Buenos_Aires),
            "Buenos Aires"
        );
        assert_eq!(city_name(Tz::UTC), "UTC");
    }

    #[test]
    fn offsets_show_sign_hours_and_leftover_minutes() {
        assert_eq!(signed_hours_and_minutes(-6 * 3600), "−6 h");
        assert_eq!(signed_hours_and_minutes(3 * 3600 + 1800), "+3 h 30 min");
        assert_eq!(signed_hours_and_minutes(0), "+0 h");
    }
}
