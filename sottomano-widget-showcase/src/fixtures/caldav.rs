use chrono::{NaiveDateTime, TimeDelta, Timelike};
use sottomano_config::SecretPaths;
use sottomano_sources::{
    SourceDelivery,
    caldav_tasks_events::{
        agenda_moment_zones::AgendaMoment,
        caldav_agenda::{CaldavAgenda, ChangeDelivery},
        caldav_event_query::UpcomingEvent,
        caldav_server_provider::CaldavProvider,
        caldav_task_query::OpenTask,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::caldav_group::{CaldavGroup, CaldavMessage},
};

use crate::showcased_group::ShowcasedGroup;

const NEXTCLOUD_SERVER: &str = "https://cloud.example.org";
const PERSONAL_CALENDAR: &str = "/remote.php/dav/calendars/demo/personal/";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = CaldavGroup::new(&SecretPaths::default());
    let _ = group.update(
        CaldavMessage::Agenda(Box::new(SourceDelivery::Reading(Ok(nextcloud_agenda(
            clock,
        ))))),
        clock,
    );
    ShowcasedGroup::Caldav(Box::new(group))
}

fn whole_hour(clock: Clock) -> NaiveDateTime {
    let local_moment = clock.local().naive_local();
    local_moment
        .with_minute(0)
        .and_then(|moment| moment.with_second(0))
        .and_then(|moment| moment.with_nanosecond(0))
        .unwrap_or(local_moment)
}

fn nextcloud_agenda(clock: Clock) -> CaldavAgenda {
    let starting_hour = whole_hour(clock);
    let minutes_from_hour =
        |minutes: i64| AgendaMoment::Floating(starting_hour + TimeDelta::minutes(minutes));
    let calendar_object_href = |summary: &str| format!("{PERSONAL_CALENDAR}{summary}.ics");
    let timed_event = |summary: &str, start: i64, end: i64, location: Option<&str>| UpcomingEvent {
        summary: summary.to_owned(),
        calendar: "Personal".to_owned(),
        object_href: calendar_object_href(summary),
        recurrence_id: minutes_from_hour(start),
        start: minutes_from_hour(start),
        end: Some(minutes_from_hour(end)),
        location: location.map(str::to_owned),
    };
    let open_task =
        |summary: &str, due: Option<i64>, priority: u32, percent_complete: u8| OpenTask {
            summary: summary.to_owned(),
            calendar: "Tasks".to_owned(),
            object_href: calendar_object_href(summary),
            due: due.map(minutes_from_hour),
            priority: Some(priority),
            percent_complete: Some(percent_complete),
            started: percent_complete > 0,
        };
    CaldavAgenda {
        events: vec![
            timed_event("Standup", -15, 15, None),
            timed_event("Design review", 120, 180, Some("Room 2.14")),
            timed_event("Climbing", 480, 600, Some("Boulderhal Gent")),
        ],
        tasks: vec![
            open_task("Renew passport", Some(-26 * 60), 1, 0),
            open_task("Review release notes", Some(5 * 60), 5, 40),
            open_task("Plan weekend trip", None, 7, 0),
        ],
        calendar_count: 2,
        change_delivery: ChangeDelivery::NextcloudNotifyPush,
        provider: CaldavProvider::detected(NEXTCLOUD_SERVER, [PERSONAL_CALENDAR]),
    }
}
