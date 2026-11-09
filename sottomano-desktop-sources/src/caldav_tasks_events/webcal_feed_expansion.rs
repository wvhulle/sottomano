use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    time::Instant,
};

use calcard::{
    common::timezone::Tz as CalendarZone,
    icalendar::{
        ICalendar, ICalendarComponent, ICalendarComponentType, ICalendarProperty, ICalendarValue,
        dates::{CalendarEvent, TimeOrDelta},
    },
};
use chrono::{DateTime, TimeDelta, Utc};
use tokio::task::spawn_blocking;

use crate::{
    caldav_tasks_events::{
        agenda_moment_zones::AgendaMoment,
        caldav_event_query::{
            EventOrigin, EventWindow, UpcomingEvent, WindowOverlap, displayed_summary,
        },
        caldav_home_listing::SubscribedCalendar,
    },
    system_protocol_clients::http_json_client,
};

const SERIES_EXPANSION_LIMIT: usize = 50_000;

type ExpandedOccurrence =
    CalendarEvent<DateTime<CalendarZone>, TimeOrDelta<DateTime<CalendarZone>, TimeDelta>>;

#[derive(Debug, thiserror::Error)]
pub enum FeedExpansionError {
    #[error("download failed: {0}")]
    Download(reqwest::Error),
    #[error("feed is not an iCalendar file")]
    Unparseable,
    #[error("feed expansion was interrupted")]
    ExpansionInterrupted,
}

impl From<reqwest::Error> for FeedExpansionError {
    fn from(error: reqwest::Error) -> Self {
        FeedExpansionError::Download(error.without_url())
    }
}

pub async fn download_feed_events(
    http_client: &reqwest::Client,
    subscription: &SubscribedCalendar,
    window: EventWindow,
) -> Result<Vec<UpcomingEvent>, FeedExpansionError> {
    let feed_text = http_client
        .get(&subscription.source_url)
        .timeout(http_json_client::REQUEST_TIMEOUT)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let calendar_name = subscription.name.clone();
    spawn_blocking(move || {
        let expansion_started = Instant::now();
        let expanded_events =
            expanded_feed_events(&feed_text, &calendar_name, window, viewer_zone());
        tracing::debug!(
            "subscription {calendar_name}: expanded {} bytes in {:?}",
            feed_text.len(),
            expansion_started.elapsed(),
        );
        expanded_events
    })
    .await
    .unwrap_or(Err(FeedExpansionError::ExpansionInterrupted))
}

pub fn expanded_feed_events(
    feed_text: &str,
    calendar_name: &str,
    window: EventWindow,
    viewer_zone: CalendarZone,
) -> Result<Vec<UpcomingEvent>, FeedExpansionError> {
    let Ok(feed) = ICalendar::parse(feed_text) else {
        return Err(FeedExpansionError::Unparseable);
    };
    Ok(series_calendars(&feed)
        .flat_map(|series| series_events_in_window(&series, calendar_name, viewer_zone, window))
        .collect())
}

fn series_calendars(feed: &ICalendar) -> impl Iterator<Item = ICalendar> + '_ {
    let series_uids: BTreeSet<&str> = feed
        .components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VEvent)
        .filter_map(ICalendarComponent::uid)
        .collect();
    let mut components_by_uid: BTreeMap<&str, Vec<&ICalendarComponent>> = BTreeMap::new();
    feed.components
        .iter()
        .filter(|component| component.component_type.has_time_ranges())
        .filter_map(|component| Some((component.uid()?, component)))
        .filter(|(uid, _)| series_uids.contains(uid))
        .for_each(|(uid, component)| components_by_uid.entry(uid).or_default().push(component));
    let shared_calendar = calendar_without_time_ranges(feed);
    components_by_uid
        .into_values()
        .map(move |series_components| {
            calendar_with_series(&shared_calendar, feed, &series_components)
        })
}

fn calendar_without_time_ranges(feed: &ICalendar) -> ICalendar {
    let component_count = u32::try_from(feed.components.len()).unwrap_or(u32::MAX);
    let time_range_ids: Vec<u32> = (0..component_count)
        .filter(|component_id| {
            feed.component_by_id(*component_id)
                .is_some_and(|component| component.component_type.has_time_ranges())
        })
        .collect();
    let mut shared_calendar = feed.clone();
    shared_calendar.remove_component_ids(&time_range_ids);
    shared_calendar
}

fn calendar_with_series(
    shared_calendar: &ICalendar,
    feed: &ICalendar,
    series_components: &[&ICalendarComponent],
) -> ICalendar {
    let mut series = shared_calendar.clone();
    for component in series_components {
        let appended_id = appended_component_id(&mut series, feed, component);
        if let Some(calendar_root) = series.components.first_mut() {
            calendar_root.component_ids.push(appended_id);
        }
    }
    series
}

fn appended_component_id(
    series: &mut ICalendar,
    feed: &ICalendar,
    component: &ICalendarComponent,
) -> u32 {
    let appended_index = series.components.len();
    series.components.push(component.clone());
    let child_ids: Vec<u32> = component
        .component_ids
        .iter()
        .filter_map(|child_id| feed.component_by_id(*child_id))
        .map(|child| appended_component_id(series, feed, child))
        .collect();
    series.components[appended_index].component_ids = child_ids;
    u32::try_from(appended_index).unwrap_or(u32::MAX)
}

fn series_events_in_window(
    series: &ICalendar,
    calendar_name: &str,
    viewer_zone: CalendarZone,
    window: EventWindow,
) -> Vec<UpcomingEvent> {
    series
        .expand_dates(viewer_zone, SERIES_EXPANSION_LIMIT)
        .events
        .into_iter()
        .filter_map(|occurrence| feed_event(series, &occurrence, calendar_name))
        .filter(|event| event.window_overlap(window) == WindowOverlap::Overlapping)
        .collect()
}

fn feed_event(
    series: &ICalendar,
    occurrence: &ExpandedOccurrence,
    calendar_name: &str,
) -> Option<UpcomingEvent> {
    let component = series.component_by_id(occurrence.comp_id)?;
    if component.component_type != ICalendarComponentType::VEvent {
        return None;
    }
    let ends_at = match occurrence.end {
        TimeOrDelta::Time(end) => end,
        TimeOrDelta::Delta(duration) => occurrence.start + duration,
    };
    let moment_of: fn(DateTime<CalendarZone>) -> AgendaMoment = match start_precision(component) {
        StartPrecision::DateOnly => all_day_moment,
        StartPrecision::DateAndTime => AgendaMoment::from,
    };
    Some(UpcomingEvent {
        summary: displayed_summary(
            text_property(component, &ICalendarProperty::Summary).unwrap_or_default(),
        ),
        calendar: calendar_name.to_owned(),
        origin: EventOrigin::SubscriptionFeed,
        start: moment_of(occurrence.start),
        end: moment_of(ends_at),
        location: text_property(component, &ICalendarProperty::Location)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    })
}

fn all_day_moment(midnight: DateTime<CalendarZone>) -> AgendaMoment {
    AgendaMoment::AllDay(midnight.date_naive())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartPrecision {
    DateOnly,
    DateAndTime,
}

fn start_precision(component: &ICalendarComponent) -> StartPrecision {
    if component
        .property(&ICalendarProperty::Dtstart)
        .and_then(|entry| entry.values.first())
        .and_then(ICalendarValue::as_partial_date_time)
        .is_some_and(|start| !start.has_time())
    {
        StartPrecision::DateOnly
    } else {
        StartPrecision::DateAndTime
    }
}

fn text_property<'component>(
    component: &'component ICalendarComponent,
    property: &ICalendarProperty,
) -> Option<&'component str> {
    component.property(property)?.values.first()?.as_text()
}

fn viewer_zone() -> CalendarZone {
    let Ok(zone_name) = iana_time_zone::get_timezone() else {
        return CalendarZone::from(Utc);
    };
    let Ok(zone) = chrono_tz::Tz::from_str(&zone_name) else {
        return CalendarZone::from(Utc);
    };
    CalendarZone::from(zone)
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};
    use chrono_tz::Europe::Brussels;

    use super::*;

    #[tokio::test]
    async fn download_failure_message_leaves_out_the_private_feed_url() {
        let refused_connection = reqwest::Client::new()
            .get("http://127.0.0.1:1/private-feed-token")
            .send()
            .await
            .unwrap_err();
        let message = FeedExpansionError::from(refused_connection).to_string();
        assert!(!message.contains("private-feed-token"), "{message}");
    }

    fn october_feed_events() -> Vec<UpcomingEvent> {
        let window = EventWindow {
            first_day: NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            days: 21,
        };
        let mut events = expanded_feed_events(
            include_str!("caldav_fixtures/outlook_subscription_feed.ics"),
            "Work",
            window,
            CalendarZone::from(Brussels),
        )
        .unwrap();
        events.sort_by_key(|event| event.start.starts_in(&Utc));
        events
    }

    fn starts_in_utc(summary: &str) -> Vec<DateTime<Utc>> {
        october_feed_events()
            .into_iter()
            .filter(|event| event.summary == summary)
            .map(|event| event.start.starts_in(&Utc))
            .collect()
    }

    #[test]
    fn a_windows_zone_series_skips_its_exdate_and_takes_its_override() {
        assert_eq!(
            starts_in_utc("Team sync"),
            [Utc.with_ymd_and_hms(2026, 10, 12, 7, 0, 0).unwrap()]
        );
        assert_eq!(
            starts_in_utc("Team sync (moved)"),
            [Utc.with_ymd_and_hms(2026, 10, 27, 9, 0, 0).unwrap()]
        );
    }

    #[test]
    fn an_unbounded_series_from_long_ago_still_reaches_the_window() {
        assert_eq!(starts_in_utc("Inbox zero").len(), 21);
    }

    #[test]
    fn an_all_day_feed_event_keeps_its_day_and_location() {
        let offsite = october_feed_events()
            .into_iter()
            .find(|event| event.summary == "Offsite")
            .unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        assert_eq!(offsite.start, AgendaMoment::AllDay(day));
        assert_eq!(offsite.end, AgendaMoment::AllDay(day.succ_opt().unwrap()));
        assert_eq!(offsite.location, "Ghent");
        assert_eq!(offsite.origin, EventOrigin::SubscriptionFeed);
    }

    fn feed_of_single_events(event_count: u32) -> String {
        let events: String = (0..event_count)
            .map(|event_number| {
                [
                    "BEGIN:VEVENT".to_owned(),
                    format!("UID:pickup-{event_number}"),
                    format!("SUMMARY:Pickup {event_number}"),
                    "DTSTART:20261012T080000Z".to_owned(),
                    "DTEND:20261012T090000Z".to_owned(),
                    "BEGIN:VALARM".to_owned(),
                    "ACTION:DISPLAY".to_owned(),
                    "TRIGGER:-PT1H".to_owned(),
                    "END:VALARM".to_owned(),
                    "END:VEVENT\n".to_owned(),
                ]
                .join("\n")
            })
            .collect();
        format!("BEGIN:VCALENDAR\nVERSION:2.0\n{events}END:VCALENDAR\n")
    }

    #[test]
    fn each_series_keeps_only_its_own_event_and_alarm() {
        let feed = ICalendar::parse(feed_of_single_events(3)).unwrap();
        let series_list: Vec<ICalendar> = series_calendars(&feed).collect();
        assert_eq!(series_list.len(), 3);
        for series in &series_list {
            let event_ids: Vec<u32> = series.components[0].component_ids.clone();
            assert_eq!(event_ids.len(), 1);
            let event = series.component_by_id(event_ids[0]).unwrap();
            assert_eq!(event.component_type, ICalendarComponentType::VEvent);
            let alarm = series.component_by_id(event.component_ids[0]).unwrap();
            assert_eq!(alarm.component_type, ICalendarComponentType::VAlarm);
        }
    }

    #[test]
    fn every_single_event_of_a_feed_expands_once() {
        let window = EventWindow {
            first_day: NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            days: 1,
        };
        let events = expanded_feed_events(
            &feed_of_single_events(3),
            "Garbage",
            window,
            CalendarZone::from(Utc),
        )
        .unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn a_feed_that_is_not_icalendar_is_rejected() {
        let window = EventWindow {
            first_day: NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            days: 7,
        };
        assert!(matches!(
            expanded_feed_events("<html></html>", "Work", window, CalendarZone::from(Utc)),
            Err(FeedExpansionError::Unparseable)
        ));
    }
}
