use chrono::{DateTime, Days, Local, NaiveDate};
use fast_dav_rs::{CalendarQueryOptions, TimeRange};
use icalendar::{Component, EventLike};

use crate::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment,
    caldav_agenda::{CalendarResource, Error, parsed_calendar_data},
    caldav_calendar_discovery::{CalendarCollection, CalendarHome},
};

const CALDAV_UTC_FORMAT: &str = "%Y%m%dT%H%M%SZ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpcomingEvent {
    pub summary: String,
    pub calendar: String,
    pub object_href: String,
    pub recurrence_id: AgendaMoment,
    pub start: AgendaMoment,
    pub end: Option<AgendaMoment>,
    pub location: Option<String>,
}

impl UpcomingEvent {
    #[must_use]
    pub fn has_ended(&self, now: DateTime<Local>) -> bool {
        match self.end {
            Some(AgendaMoment::AllDay(last_day_exclusive)) => {
                last_day_exclusive <= now.date_naive()
            }
            Some(end) => end.starts_at() <= now,
            None => self.start.ends_before(now),
        }
    }

    #[must_use]
    pub fn is_ongoing(&self, now: DateTime<Local>) -> bool {
        self.start.starts_at() <= now && !self.has_ended(now)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventWindow {
    pub first_day: NaiveDate,
    pub days: u32,
}

impl EventWindow {
    fn caldav_bounds(self) -> Option<TimeRange> {
        let last_day_exclusive = self
            .first_day
            .checked_add_days(Days::new(u64::from(self.days)))?;
        let [start, end] = [self.first_day, last_day_exclusive].map(|day| {
            AgendaMoment::AllDay(day)
                .starts_in(&Local)
                .to_utc()
                .format(CALDAV_UTC_FORMAT)
                .to_string()
        });
        Some(TimeRange::new(start).with_end(end))
    }

    fn query_options(self) -> Option<CalendarQueryOptions> {
        let bounds = self.caldav_bounds()?;
        Some(
            CalendarQueryOptions::new("VEVENT")
                .with_start(bounds.start.clone())
                .with_end(bounds.end.clone()?)
                .with_expand(bounds),
        )
    }
}

pub async fn query_upcoming_events(
    home: &CalendarHome,
    calendar: &CalendarCollection,
    window: EventWindow,
) -> Result<Vec<UpcomingEvent>, Error> {
    let options = window
        .query_options()
        .ok_or(Error::Discovery("upcoming_days reaches past the calendar"))?;
    let objects = home
        .client
        .calendar_query_options(&calendar.href, &options)
        .await?;
    Ok(upcoming_events(
        &parsed_calendar_data(objects),
        &calendar.name,
    ))
}

fn upcoming_events(resources: &[CalendarResource], calendar_name: &str) -> Vec<UpcomingEvent> {
    resources
        .iter()
        .flat_map(|resource| {
            resource
                .calendar
                .events()
                .map(move |event| (resource, event))
        })
        .filter_map(|(resource, event)| {
            let start = AgendaMoment::of(event.get_start()?);
            Some(UpcomingEvent {
                summary: event
                    .get_summary()
                    .unwrap_or("(untitled)")
                    .trim()
                    .to_owned(),
                calendar: calendar_name.to_owned(),
                object_href: resource.href.clone(),
                recurrence_id: event.get_recurrence_id().map_or(start, AgendaMoment::of),
                start,
                end: event.get_end().map(AgendaMoment::of),
                location: event
                    .get_location()
                    .map(str::trim)
                    .filter(|location| !location.is_empty())
                    .map(str::to_owned),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::{TimeDelta, TimeZone, Utc};
    use icalendar::Calendar;

    use super::*;

    fn parsed(ics: &str) -> Vec<UpcomingEvent> {
        let resource = CalendarResource {
            href: "/remote.php/dav/calendars/wvhulle/personal/fixture.ics".to_owned(),
            calendar: Calendar::from_str(ics).unwrap(),
        };
        upcoming_events(&[resource], "Personal")
    }

    #[test]
    fn an_expanded_recurrence_yields_one_event_per_occurrence() {
        let events = parsed(include_str!("caldav_fixtures/expanded_weekly_standup.ics"));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].summary, "Standup");
        assert_eq!(events[0].location.as_deref(), Some("Room 4"));
        let second_start = Utc.with_ymd_and_hms(2026, 11, 2, 9, 0, 0).unwrap();
        assert_eq!(events[1].start.starts_in(&Utc), second_start);
        assert_eq!(events[1].recurrence_id, events[1].start);
        assert!(events[1].object_href.ends_with("/fixture.ics"));
    }

    #[test]
    fn a_single_event_is_referenced_by_its_start() {
        let holiday = &parsed(include_str!("caldav_fixtures/all_day_holiday.ics"))[0];
        assert_eq!(holiday.recurrence_id, holiday.start);
    }

    #[test]
    fn an_all_day_event_ends_when_its_exclusive_end_day_begins() {
        let holiday = &parsed(include_str!("caldav_fixtures/all_day_holiday.ics"))[0];
        let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(holiday.start, AgendaMoment::AllDay(day));
        let noon = Local
            .from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
            .unwrap();
        assert!(holiday.is_ongoing(noon));
        assert!(holiday.has_ended(noon + TimeDelta::days(1)));
    }

    #[test]
    fn the_query_window_expands_over_whole_local_days() {
        let window = EventWindow {
            first_day: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
            days: 7,
        };
        let options = window.query_options().unwrap();
        let expand = options.expand.unwrap();
        assert_eq!(options.start.as_deref(), Some(expand.start.as_str()));
        assert_eq!(options.end, expand.end);
        assert!(expand.start.ends_with('Z'));
    }
}
