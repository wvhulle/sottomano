use std::str::FromStr;

use calcard::{
    common::{DateTimeResult, timezone::Tz as CalendarZone},
    icalendar::ICalendarEntry,
};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Tz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgendaMoment {
    AllDay(NaiveDate),
    Floating(NaiveDateTime),
    Zoned(DateTime<Tz>),
}

impl AgendaMoment {
    #[must_use]
    pub fn from_calendar_entry(entry: &ICalendarEntry) -> Option<Self> {
        let written_moment = entry.values.first()?.as_partial_date_time()?;
        let DateTimeResult { date_time, offset } = written_moment.to_date_time()?;
        if !written_moment.has_time() {
            return Some(AgendaMoment::AllDay(date_time.date()));
        }
        if let Some(utc_offset) = offset {
            let instant = utc_offset.from_local_datetime(&date_time).single()?;
            return Some(AgendaMoment::Zoned(instant.with_timezone(&Tz::UTC)));
        }
        let Some(zone) = entry.tz_id().and_then(resolved_zone) else {
            return Some(AgendaMoment::Floating(date_time));
        };
        let Some(instant) = zone.from_local_datetime(&date_time).earliest() else {
            return Some(AgendaMoment::Floating(date_time));
        };
        Some(AgendaMoment::Zoned(instant))
    }

    #[must_use]
    pub fn default_event_end(self) -> Self {
        match self {
            AgendaMoment::AllDay(day) => day.succ_opt().map_or(self, AgendaMoment::AllDay),
            AgendaMoment::Floating(_) | AgendaMoment::Zoned(_) => self,
        }
    }

    #[must_use]
    pub fn starts_in<Z: TimeZone>(self, viewer: &Z) -> DateTime<Z> {
        match self {
            AgendaMoment::AllDay(day) => wall_clock_in(viewer, day.and_time(NaiveTime::MIN)),
            AgendaMoment::Floating(wall_clock) => wall_clock_in(viewer, wall_clock),
            AgendaMoment::Zoned(instant) => instant.with_timezone(viewer),
        }
    }

    #[must_use]
    pub fn starts_at(self) -> DateTime<Local> {
        self.starts_in(&Local)
    }

    #[must_use]
    pub fn day_in<Z: TimeZone>(self, viewer: &Z) -> NaiveDate {
        match self {
            AgendaMoment::AllDay(day) => day,
            AgendaMoment::Floating(_) | AgendaMoment::Zoned(_) => {
                self.starts_in(viewer).date_naive()
            }
        }
    }

    #[must_use]
    pub fn passage_at(self, now: DateTime<Local>) -> DeadlinePassage {
        let passed = match self {
            AgendaMoment::AllDay(day) => day < now.date_naive(),
            AgendaMoment::Floating(_) | AgendaMoment::Zoned(_) => self.starts_at() < now,
        };
        if passed {
            DeadlinePassage::Passed
        } else {
            DeadlinePassage::Ahead
        }
    }

    #[must_use]
    pub fn source_zone(self) -> Option<Tz> {
        match self {
            AgendaMoment::Zoned(instant) => Some(instant.timezone()),
            AgendaMoment::AllDay(_) | AgendaMoment::Floating(_) => None,
        }
    }
}

impl From<DateTime<CalendarZone>> for AgendaMoment {
    fn from(instant: DateTime<CalendarZone>) -> Self {
        match instant.timezone() {
            CalendarZone::Floating => AgendaMoment::Floating(instant.naive_local()),
            CalendarZone::Fixed(_) => AgendaMoment::Zoned(instant.with_timezone(&Tz::UTC)),
            CalendarZone::Tz(zone) => AgendaMoment::Zoned(instant.with_timezone(&zone)),
        }
    }
}

fn resolved_zone(tzid: &str) -> Option<Tz> {
    match CalendarZone::from_str(tzid) {
        Ok(CalendarZone::Tz(zone)) => Some(zone),
        Ok(CalendarZone::Floating | CalendarZone::Fixed(_)) | Err(()) => None,
    }
}

fn wall_clock_in<Z: TimeZone>(viewer: &Z, wall_clock: NaiveDateTime) -> DateTime<Z> {
    viewer
        .from_local_datetime(&wall_clock)
        .earliest()
        .unwrap_or_else(|| viewer.from_utc_datetime(&wall_clock))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlinePassage {
    Passed,
    Ahead,
}

#[cfg(test)]
mod tests {
    use calcard::icalendar::{ICalendar, ICalendarProperty};
    use chrono::{NaiveDate, TimeZone, Utc};
    use chrono_tz::{America::New_York, Europe::Brussels};

    use super::*;

    fn wall_clock(hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 30)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    fn written_start(dtstart_line: &str) -> AgendaMoment {
        let calendar = ICalendar::parse(format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:fixture\r\n{dtstart_line}\r\nEND:VEVENT\r\\
             nEND:VCALENDAR\r\n"
        ))
        .unwrap();
        calendar
            .components
            .iter()
            .find_map(|component| component.property(&ICalendarProperty::Dtstart))
            .and_then(AgendaMoment::from_calendar_entry)
            .unwrap()
    }

    #[test]
    fn a_zoned_time_keeps_its_source_zone_and_instant() {
        let moment = written_start("DTSTART;TZID=Europe/Brussels:20261030T140000");
        assert_eq!(moment.source_zone(), Some(Brussels));
        assert_eq!(
            moment.starts_in(&Utc),
            Utc.with_ymd_and_hms(2026, 10, 30, 13, 0, 0).unwrap()
        );
        assert_eq!(moment.starts_in(&New_York).naive_local(), wall_clock(9));
    }

    #[test]
    fn a_windows_zone_name_resolves_to_its_iana_zone() {
        let moment = written_start("DTSTART;TZID=W. Europe Standard Time:20261030T140000");
        assert_eq!(
            moment.starts_in(&Utc),
            Utc.with_ymd_and_hms(2026, 10, 30, 13, 0, 0).unwrap()
        );
    }

    #[test]
    fn an_unknown_zone_name_falls_back_to_the_viewers_wall_clock() {
        let moment = written_start("DTSTART;TZID=Olympus Mons Standard Time:20261030T140000");
        assert_eq!(moment, AgendaMoment::Floating(wall_clock(14)));
        assert_eq!(moment.starts_in(&New_York).naive_local(), wall_clock(14));
    }

    #[test]
    fn a_time_without_zone_is_floating() {
        assert_eq!(
            written_start("DTSTART:20261030T140000"),
            AgendaMoment::Floating(wall_clock(14))
        );
    }

    #[test]
    fn a_date_value_is_an_all_day_moment() {
        assert_eq!(
            written_start("DTSTART;VALUE=DATE:20261030"),
            AgendaMoment::AllDay(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap())
        );
    }

    #[test]
    fn a_utc_time_is_zoned_in_utc() {
        let moment = written_start("DTSTART:20261030T233000Z");
        assert_eq!(moment.source_zone(), Some(Tz::UTC));
        assert_eq!(
            moment.day_in(&Brussels),
            NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()
        );
    }

    #[test]
    fn an_all_day_moment_starts_at_the_viewers_midnight() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let moment = AgendaMoment::AllDay(day);
        assert_eq!(
            moment.starts_in(&New_York).naive_local(),
            day.and_time(NaiveTime::MIN)
        );
        assert_eq!(moment.day_in(&Brussels), day);
    }
}
