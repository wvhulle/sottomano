use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use calcard::{
    common::timezone::Tz as CalendarZone,
    icalendar::{
        ICalendar, ICalendarComponent, ICalendarComponentType, ICalendarProperty, ICalendarValue,
        dates::{CalendarEvent, TimeOrDelta},
    },
};
use chrono::{DateTime, TimeDelta, Utc};

use crate::{
    caldav_tasks_events::{
        agenda_moment_zones::AgendaMoment,
        caldav_event_query::{EventOrigin, EventWindow, UpcomingEvent, displayed_summary},
        caldav_home_listing::SubscribedCalendar,
    },
    system_protocol_clients::http_json_client,
};

const SERIES_EXPANSION_LIMIT: usize = 50_000;

type ExpandedOccurrence =
    CalendarEvent<DateTime<CalendarZone>, TimeOrDelta<DateTime<CalendarZone>, TimeDelta>>;

#[derive(Debug)]
pub enum FeedExpansionError {
    Download(reqwest::Error),
    Unparseable,
}

impl Display for FeedExpansionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            FeedExpansionError::Download(error) => write!(f, "download failed: {error}"),
            FeedExpansionError::Unparseable => write!(f, "feed is not an iCalendar file"),
        }
    }
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
    expanded_feed_events(&feed_text, &subscription.name, window, viewer_zone())
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
    let series_uids: BTreeSet<&str> = feed
        .components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VEvent)
        .filter_map(ICalendarComponent::uid)
        .collect();
    Ok(series_uids
        .into_iter()
        .map(|uid| series_calendar(&feed, uid))
        .flat_map(|series| series_events(&series, calendar_name, viewer_zone))
        .filter(|event| event.overlaps(window))
        .collect())
}

fn series_calendar(feed: &ICalendar, uid: &str) -> ICalendar {
    let component_count = u32::try_from(feed.components.len()).unwrap_or(u32::MAX);
    let other_series_ids: Vec<u32> = (0..component_count)
        .filter(|component_id| {
            feed.component_by_id(*component_id)
                .is_some_and(|component| belongs_to_other_series(component, uid))
        })
        .collect();
    let mut series = feed.clone();
    series.remove_component_ids(&other_series_ids);
    series
}

fn belongs_to_other_series(component: &ICalendarComponent, uid: &str) -> bool {
    component.component_type.has_time_ranges() && component.uid() != Some(uid)
}

fn series_events(
    series: &ICalendar,
    calendar_name: &str,
    viewer_zone: CalendarZone,
) -> Vec<UpcomingEvent> {
    series
        .expand_dates(viewer_zone, SERIES_EXPANSION_LIMIT)
        .events
        .into_iter()
        .filter_map(|occurrence| feed_event(series, &occurrence, calendar_name))
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
    let moment_of: fn(DateTime<CalendarZone>) -> AgendaMoment = if starts_on_date(component) {
        all_day_moment
    } else {
        AgendaMoment::of_expanded
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

fn starts_on_date(component: &ICalendarComponent) -> bool {
    component
        .property(&ICalendarProperty::Dtstart)
        .and_then(|entry| entry.values.first())
        .and_then(ICalendarValue::as_partial_date_time)
        .is_some_and(|start| !start.has_time())
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
