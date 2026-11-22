use calcard::icalendar::{ICalendarComponent, ICalendarComponentType, ICalendarProperty};
use chrono::{DateTime, Days, Local, NaiveDate};
use libdav::Depth;

use crate::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment,
    caldav_agenda::{CalendarResource, Error, parsed_calendar_data},
    caldav_calendar_discovery::{CalendarCollection, CalendarHome},
    caldav_webdav_requests::{DavMethod, MultistatusRequest, parsed_calendar_objects},
    webcal_feed_expansion::text_property,
};

const CALDAV_UTC_FORMAT: &str = "%Y%m%dT%H%M%SZ";
const UNTITLED_SUMMARY: &str = "(untitled)";
const EXPANDED_EVENT_QUERY_TEMPLATE: &str =
    include_str!("caldav_request_bodies/expanded_event_query.xml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventOrigin {
    CalendarObject {
        object_href: String,
        recurrence_id: AgendaMoment,
    },
    SubscriptionFeed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpcomingEvent {
    pub summary: String,
    pub calendar: String,
    pub origin: EventOrigin,
    pub start: AgendaMoment,
    pub end: AgendaMoment,
    pub location: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Upcoming,
    Ongoing,
    Ended,
}

impl UpcomingEvent {
    #[must_use]
    pub fn phase(&self, now: DateTime<Local>) -> EventPhase {
        let ended = match self.end {
            AgendaMoment::AllDay(last_day_exclusive) => last_day_exclusive <= now.date_naive(),
            AgendaMoment::Floating(_) | AgendaMoment::Zoned(_) => self.end.starts_at() <= now,
        };
        if ended {
            EventPhase::Ended
        } else if self.start.starts_at() <= now {
            EventPhase::Ongoing
        } else {
            EventPhase::Upcoming
        }
    }

    #[must_use]
    pub fn window_overlap(&self, window: EventWindow) -> WindowOverlap {
        match window.last_day_exclusive() {
            Some(window_end)
                if self.start.day_in(&Local) < window_end
                    && self.end.day_in(&Local) >= window.first_day =>
            {
                WindowOverlap::Overlapping
            }
            Some(_) | None => WindowOverlap::Outside,
        }
    }
}

pub(super) fn displayed_summary(summary: &str) -> String {
    match summary.trim() {
        "" => UNTITLED_SUMMARY.to_owned(),
        trimmed => trimmed.to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventWindow {
    pub first_day: NaiveDate,
    pub days: u32,
}

impl EventWindow {
    fn last_day_exclusive(self) -> Option<NaiveDate> {
        self.first_day
            .checked_add_days(Days::new(u64::from(self.days)))
    }

    fn caldav_bounds(self) -> Option<CaldavTimeRange> {
        let last_day_exclusive = self.last_day_exclusive()?;
        let [start, end] = [self.first_day, last_day_exclusive].map(|day| {
            AgendaMoment::AllDay(day)
                .starts_in(&Local)
                .to_utc()
                .format(CALDAV_UTC_FORMAT)
                .to_string()
        });
        Some(CaldavTimeRange { start, end })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaldavTimeRange {
    start: String,
    end: String,
}

fn expanded_event_query(CaldavTimeRange { start, end }: &CaldavTimeRange) -> String {
    EXPANDED_EVENT_QUERY_TEMPLATE
        .replace("{start}", start)
        .replace("{end}", end)
}

pub async fn query_upcoming_events(
    home: &CalendarHome,
    calendar: &CalendarCollection,
    window: EventWindow,
) -> Result<Vec<UpcomingEvent>, Error> {
    let Some(bounds) = window.caldav_bounds() else {
        return Err(Error::WindowBeyondCalendar);
    };
    let objects = home
        .multistatus_request(MultistatusRequest {
            method: DavMethod::Report,
            path: &calendar.href,
            depth: Depth::One,
            xml_body: expanded_event_query(&bounds),
            parse_multistatus: parsed_calendar_objects,
        })
        .await?;
    let object_count = objects.len();
    let calendar_resources = parsed_calendar_data(objects);
    let events = upcoming_events(&calendar_resources, &calendar.name);
    tracing::debug!(
        "calendar {}: {object_count} objects, {} parsed, {} events",
        calendar.name,
        calendar_resources.len(),
        events.len(),
    );
    Ok(events)
}

fn upcoming_events(resources: &[CalendarResource], calendar_name: &str) -> Vec<UpcomingEvent> {
    resources
        .iter()
        .flat_map(|resource| {
            resource
                .calendar
                .components
                .iter()
                .filter(|component| component.component_type == ICalendarComponentType::VEvent)
                .filter_map(move |event| {
                    calendar_object_event(&resource.href, event, calendar_name)
                })
        })
        .collect()
}

fn calendar_object_event(
    object_href: &str,
    event: &ICalendarComponent,
    calendar_name: &str,
) -> Option<UpcomingEvent> {
    let Some(start) = written_moment(event, &ICalendarProperty::Dtstart) else {
        tracing::warn!("calendar object {object_href}: event has no readable start");
        return None;
    };
    Some(UpcomingEvent {
        summary: displayed_summary(
            text_property(event, &ICalendarProperty::Summary).unwrap_or_default(),
        ),
        calendar: calendar_name.to_owned(),
        origin: EventOrigin::CalendarObject {
            object_href: object_href.to_owned(),
            recurrence_id: written_moment(event, &ICalendarProperty::RecurrenceId).unwrap_or(start),
        },
        start,
        end: written_moment(event, &ICalendarProperty::Dtend)
            .unwrap_or_else(|| start.default_event_end()),
        location: text_property(event, &ICalendarProperty::Location)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    })
}

pub(super) fn written_moment(
    component: &ICalendarComponent,
    property: &ICalendarProperty,
) -> Option<AgendaMoment> {
    let entry = component.property(property)?;
    AgendaMoment::try_from(entry)
        .inspect_err(|error| tracing::warn!("calendar property {property:?}: {error}"))
        .ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowOverlap {
    Overlapping,
    Outside,
}

#[cfg(test)]
mod tests {
    use calcard::icalendar::ICalendar;
    use chrono::{TimeDelta, TimeZone, Utc};

    use super::*;

    fn parsed(ics: &str) -> Vec<UpcomingEvent> {
        let resource = CalendarResource {
            href: "/remote.php/dav/calendars/wvhulle/personal/fixture.ics".to_owned(),
            calendar: ICalendar::parse(ics).unwrap(),
        };
        upcoming_events(&[resource], "Personal")
    }

    #[test]
    fn a_calendar_query_report_becomes_upcoming_events() {
        let objects =
            parsed_calendar_objects(include_str!("caldav_fixtures/calendar_query_report.xml"))
                .unwrap();
        let events = upcoming_events(&parsed_calendar_data(objects), "Personal");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "Standup");
        assert_eq!(events[0].calendar, "Personal");
    }

    #[test]
    fn an_expanded_recurrence_yields_one_event_per_occurrence() {
        let events = parsed(include_str!("caldav_fixtures/expanded_weekly_standup.ics"));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].summary, "Standup");
        assert_eq!(events[0].location, "Room 4");
        let second_start = Utc.with_ymd_and_hms(2026, 11, 2, 9, 0, 0).unwrap();
        assert_eq!(events[1].start.starts_in(&Utc), second_start);
        let EventOrigin::CalendarObject {
            object_href,
            recurrence_id,
        } = &events[1].origin
        else {
            panic!("a CalDAV event comes from a calendar object");
        };
        assert_eq!(*recurrence_id, events[1].start);
        assert!(object_href.ends_with("/fixture.ics"));
    }

    #[test]
    fn a_single_event_is_referenced_by_its_start() {
        let holiday = &parsed(include_str!("caldav_fixtures/all_day_holiday.ics"))[0];
        assert!(matches!(
            holiday.origin,
            EventOrigin::CalendarObject { recurrence_id, .. } if recurrence_id == holiday.start
        ));
    }

    #[test]
    fn an_event_without_end_lasts_its_whole_start_day() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(
            AgendaMoment::AllDay(day).default_event_end(),
            AgendaMoment::AllDay(day.succ_opt().unwrap())
        );
    }

    #[test]
    fn an_all_day_event_ends_when_its_exclusive_end_day_begins() {
        let holiday = &parsed(include_str!("caldav_fixtures/all_day_holiday.ics"))[0];
        let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(holiday.start, AgendaMoment::AllDay(day));
        let noon = Local
            .from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap())
            .unwrap();
        assert_eq!(holiday.phase(noon), EventPhase::Ongoing);
        assert_eq!(holiday.phase(noon + TimeDelta::days(1)), EventPhase::Ended);
    }

    #[test]
    fn the_query_window_expands_over_whole_local_days() {
        let window = EventWindow {
            first_day: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
            days: 7,
        };
        let bounds = window.caldav_bounds().unwrap();
        assert_eq!(bounds.start.len(), "20261024T000000Z".len());
        assert!(bounds.start.ends_with('Z') && bounds.end.ends_with('Z'));
        assert!(bounds.start < bounds.end);
        let query = expanded_event_query(&bounds);
        assert!(query.contains(&format!(
            r#"<c:expand start="{}" end="{}"/>"#,
            bounds.start, bounds.end
        )));
        assert!(query.contains(&format!(
            r#"<c:time-range start="{}" end="{}"/>"#,
            bounds.start, bounds.end
        )));
    }
}
