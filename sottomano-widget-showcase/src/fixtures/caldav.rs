use std::time::Duration;

use chrono::{NaiveDateTime, TimeDelta, Timelike};
use sottomano_config::SecretPaths;
use sottomano_sources::{
    SnapshotDelivery,
    caldav_tasks_events::{
        agenda_moment_zones::AgendaMoment,
        caldav_agenda::CaldavAgenda,
        caldav_event_query::{EventOrigin, UpcomingEvent},
        caldav_server_provider::CaldavProvider,
        caldav_task_query::{OpenTask, TaskDeadline, TaskPriority, TaskProgress},
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
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
        CaldavMessage::Agenda(Box::new(SnapshotDelivery::fixture_delivered(
            Ok(nextcloud_agenda(clock)),
            DeliveryCadence::LivePushWithRecheck(PeriodicPolling::every(Duration::from_mins(15))),
        ))),
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
    let timed_event = |summary: &str, start: i64, end: i64, location: &str| UpcomingEvent {
        summary: summary.to_owned(),
        calendar: "Personal".to_owned(),
        origin: EventOrigin::CalendarObject {
            object_href: calendar_object_href(summary),
            recurrence_id: minutes_from_hour(start),
        },
        start: minutes_from_hour(start),
        end: minutes_from_hour(end),
        location: location.to_owned(),
    };
    let open_task =
        |summary: &str, deadline: TaskDeadline, priority: u32, progress: TaskProgress| OpenTask {
            summary: summary.to_owned(),
            calendar: "Tasks".to_owned(),
            object_href: calendar_object_href(summary),
            deadline,
            priority: TaskPriority::Ranked(priority),
            progress,
        };
    let due_in_minutes = |minutes: i64| TaskDeadline::Due(minutes_from_hour(minutes));
    CaldavAgenda {
        events: vec![
            timed_event("Standup", -15, 15, ""),
            timed_event("Design review", 120, 180, "Room 2.14"),
            timed_event("Book club", 480, 600, "City library"),
        ],
        tasks: vec![
            open_task(
                "Renew passport",
                due_in_minutes(-26 * 60),
                1,
                TaskProgress::NotStarted,
            ),
            open_task(
                "Review release notes",
                due_in_minutes(5 * 60),
                5,
                TaskProgress::PercentComplete(40),
            ),
            open_task(
                "Plan weekend trip",
                TaskDeadline::Undated,
                7,
                TaskProgress::NotStarted,
            ),
        ],
        calendar_count: 2,
        provider: CaldavProvider::detected(NEXTCLOUD_SERVER, [PERSONAL_CALENDAR]),
    }
}
