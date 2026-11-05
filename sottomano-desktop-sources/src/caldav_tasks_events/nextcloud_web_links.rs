use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::NaiveTime;
use form_urlencoded::byte_serialize;
use reqwest::Url;

use crate::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment,
    caldav_event_query::{EventOrigin, UpcomingEvent},
    caldav_task_query::OpenTask,
};

const NEXTCLOUD_DAV_PATH: &str = "/remote.php/dav/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextcloudWebRoot {
    root: String,
}

impl NextcloudWebRoot {
    #[must_use]
    pub fn detected(server_url: &str, calendar_href: &str) -> Option<Self> {
        let (install_path, _) = calendar_href.split_once(NEXTCLOUD_DAV_PATH)?;
        let origin = Url::parse(server_url).ok()?.origin().ascii_serialization();
        Some(Self {
            root: format!("{origin}{install_path}"),
        })
    }

    #[must_use]
    pub fn calendar_app(&self) -> String {
        format!("{}/apps/calendar", self.root)
    }

    #[must_use]
    pub fn tasks_app(&self) -> String {
        format!("{}/apps/tasks", self.root)
    }

    #[must_use]
    pub fn event_details(&self, event: &UpcomingEvent) -> Option<String> {
        let EventOrigin::CalendarObject {
            object_href,
            recurrence_id,
        } = &event.origin
        else {
            return None;
        };
        let object_id: String = byte_serialize(STANDARD.encode(object_href).as_bytes()).collect();
        Some(format!(
            "{}/edit/{object_id}/{}",
            self.calendar_app(),
            calendar_js_unix_seconds(*recurrence_id)
        ))
    }

    #[must_use]
    pub fn task_details(&self, task: &OpenTask) -> Option<String> {
        let mut segments = task
            .object_href
            .rsplit('/')
            .filter(|segment| !segment.is_empty());
        let task_file = segments.next()?;
        let calendar_uri = segments.next()?;
        Some(format!(
            "{}/calendars/{calendar_uri}/tasks/{task_file}",
            self.tasks_app()
        ))
    }
}

fn calendar_js_unix_seconds(recurrence_id: AgendaMoment) -> i64 {
    match recurrence_id {
        AgendaMoment::AllDay(day) => day.and_time(NaiveTime::MIN).and_utc().timestamp(),
        AgendaMoment::Floating(wall_clock) => wall_clock.and_utc().timestamp(),
        AgendaMoment::Zoned(instant) => instant.timestamp(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use chrono_tz::Tz;

    use super::*;
    use crate::caldav_tasks_events::caldav_task_query::{TaskDeadline, TaskPriority, TaskProgress};

    fn ryzen() -> NextcloudWebRoot {
        NextcloudWebRoot::detected(
            "https://ryzen.tail1b1a03.ts.net",
            "/remote.php/dav/calendars/wvhulle/personal/",
        )
        .unwrap()
    }

    #[test]
    fn a_server_without_nextcloud_dav_paths_has_no_web_root() {
        assert_eq!(
            NextcloudWebRoot::detected("https://dav.example", "/dav/calendars/user/home/"),
            None
        );
    }

    #[test]
    fn a_nextcloud_in_a_subdirectory_keeps_its_install_path() {
        let root = NextcloudWebRoot::detected(
            "https://example.org/cloud/",
            "/cloud/remote.php/dav/calendars/user/home/",
        )
        .unwrap();
        assert_eq!(root.tasks_app(), "https://example.org/cloud/apps/tasks");
    }

    #[test]
    fn an_event_links_to_its_occurrence_in_the_calendar_app() {
        let start = AgendaMoment::Zoned(
            Utc.with_ymd_and_hms(2026, 10, 26, 8, 0, 0)
                .unwrap()
                .with_timezone(&Tz::UTC),
        );
        let event = UpcomingEvent {
            summary: "Standup".to_owned(),
            calendar: "Personal".to_owned(),
            origin: EventOrigin::CalendarObject {
                object_href: "/remote.php/dav/calendars/wvhulle/personal/standup.ics".to_owned(),
                recurrence_id: start,
            },
            start,
            end: start,
            location: String::new(),
        };
        assert_eq!(
            ryzen().event_details(&event).unwrap(),
            "https://ryzen.tail1b1a03.ts.net/apps/calendar/edit/\
             L3JlbW90ZS5waHAvZGF2L2NhbGVuZGFycy93dmh1bGxlL3BlcnNvbmFsL3N0YW5kdXAuaWNz/1793001600"
        );
        let subscribed = UpcomingEvent {
            origin: EventOrigin::SubscriptionFeed,
            ..event
        };
        assert_eq!(ryzen().event_details(&subscribed), None);
    }

    #[test]
    fn an_all_day_occurrence_is_identified_at_utc_midnight_like_calendar_js() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(
            calendar_js_unix_seconds(AgendaMoment::AllDay(day)),
            Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
    }

    #[test]
    fn a_task_links_to_its_list_and_file_in_the_tasks_app() {
        let task = OpenTask {
            summary: "Arabisch leren".to_owned(),
            calendar: "Personal Tasks".to_owned(),
            object_href: "/remote.php/dav/calendars/wvhulle/E0C13D62/arabisch.ics".to_owned(),
            deadline: TaskDeadline::Undated,
            priority: TaskPriority::Unranked,
            progress: TaskProgress::NotStarted,
        };
        assert_eq!(
            ryzen().task_details(&task).unwrap(),
            "https://ryzen.tail1b1a03.ts.net/apps/tasks/calendars/E0C13D62/tasks/arabisch.ics"
        );
    }
}
