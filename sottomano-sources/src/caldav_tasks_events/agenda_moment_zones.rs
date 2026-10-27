use std::str::FromStr;

use calcard::common::timezone::Tz as CalendarZone;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Tz;
use icalendar::{CalendarDateTime, DatePerhapsTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgendaMoment {
    AllDay(NaiveDate),
    Floating(NaiveDateTime),
    Zoned(DateTime<Tz>),
}

impl AgendaMoment {
    #[must_use]
    pub fn of(moment: DatePerhapsTime) -> Self {
        match moment {
            DatePerhapsTime::Date(day) => AgendaMoment::AllDay(day),
            DatePerhapsTime::DateTime(CalendarDateTime::Floating(wall_clock)) => {
                AgendaMoment::Floating(wall_clock)
            }
            DatePerhapsTime::DateTime(CalendarDateTime::Utc(instant)) => {
                AgendaMoment::Zoned(instant.with_timezone(&Tz::UTC))
            }
            DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone { date_time, tzid }) => {
                let Some(zone) = resolved_zone(&tzid) else {
                    return AgendaMoment::Floating(date_time);
                };
                let Some(instant) = zone.from_local_datetime(&date_time).earliest() else {
                    return AgendaMoment::Floating(date_time);
                };
                AgendaMoment::Zoned(instant)
            }
        }
    }

    #[must_use]
    pub fn of_expanded(instant: DateTime<CalendarZone>) -> Self {
        match instant.timezone() {
            CalendarZone::Floating => AgendaMoment::Floating(instant.naive_local()),
            CalendarZone::Fixed(_) => AgendaMoment::Zoned(instant.with_timezone(&Tz::UTC)),
            CalendarZone::Tz(zone) => AgendaMoment::Zoned(instant.with_timezone(&zone)),
        }
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
    pub fn is_all_day(self) -> bool {
        matches!(self, AgendaMoment::AllDay(_))
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
    pub fn ends_before(self, now: DateTime<Local>) -> bool {
        match self {
            AgendaMoment::AllDay(day) => day < now.date_naive(),
            AgendaMoment::Floating(_) | AgendaMoment::Zoned(_) => self.starts_at() < now,
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

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use chrono_tz::{America::New_York, Europe::Brussels};
    use icalendar::CalendarDateTime;

    use super::*;

    fn wall_clock(hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 30)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    #[test]
    fn a_zoned_time_keeps_its_source_zone_and_instant() {
        let moment = AgendaMoment::of(DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
            date_time: wall_clock(14),
            tzid: "Europe/Brussels".to_owned(),
        }));
        assert_eq!(moment.source_zone(), Some(Brussels));
        assert_eq!(
            moment.starts_in(&Utc),
            Utc.with_ymd_and_hms(2026, 10, 30, 13, 0, 0).unwrap()
        );
        assert_eq!(moment.starts_in(&New_York).naive_local(), wall_clock(9));
    }

    #[test]
    fn a_windows_zone_name_resolves_to_its_iana_zone() {
        let moment = AgendaMoment::of(DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
            date_time: wall_clock(14),
            tzid: "W. Europe Standard Time".to_owned(),
        }));
        assert_eq!(
            moment.starts_in(&Utc),
            Utc.with_ymd_and_hms(2026, 10, 30, 13, 0, 0).unwrap()
        );
    }

    #[test]
    fn an_unknown_zone_name_falls_back_to_the_viewers_wall_clock() {
        let moment = AgendaMoment::of(DatePerhapsTime::DateTime(CalendarDateTime::WithTimezone {
            date_time: wall_clock(14),
            tzid: "Olympus Mons Standard Time".to_owned(),
        }));
        assert_eq!(moment, AgendaMoment::Floating(wall_clock(14)));
        assert_eq!(moment.starts_in(&New_York).naive_local(), wall_clock(14));
    }

    #[test]
    fn a_utc_time_is_zoned_in_utc() {
        let instant = Utc.with_ymd_and_hms(2026, 10, 30, 23, 30, 0).unwrap();
        let moment = AgendaMoment::of(DatePerhapsTime::DateTime(CalendarDateTime::Utc(instant)));
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
