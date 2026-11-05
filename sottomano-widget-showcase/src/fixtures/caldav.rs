use std::time::Duration;

use chrono::{NaiveDateTime, TimeDelta, Timelike};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    caldav_tasks_events::{
        agenda_moment_zones::AgendaMoment,
        caldav_event_query::{EventOrigin, UpcomingEvent},
        caldav_open_tasks::OpenTaskList,
        caldav_server_provider::CaldavProvider,
        caldav_task_query::{OpenTask, TaskDeadline, TaskPriority, TaskProgress},
        caldav_upcoming_events::UpcomingEventList,
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_toml_config::SecretPaths;
use sottomano_widgets::{
    clock::Clock,
    widget_groups::caldav_group::{CaldavGroup, CaldavMessage},
};

use crate::showcased_group::ShowcasedGroup;

const NEXTCLOUD_SERVER: &str = "https://cloud.example.org";
const PERSONAL_CALENDAR: &str = "/remote.php/dav/calendars/demo/personal/";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = CaldavGroup::new(&SecretPaths::default());
    let push_cadence =
        DeliveryCadence::LivePushWithRecheck(PeriodicPolling::every(Duration::from_mins(15)));
    let _ = group.update(
        CaldavMessage::UpcomingEvents(Box::new(SnapshotDelivery::fixture_delivered(
            Ok(nextcloud_events(clock)),
            push_cadence,
        ))),
        clock,
    );
    let _ = group.update(
        CaldavMessage::OpenTasks(Box::new(SnapshotDelivery::fixture_delivered(
            Ok(nextcloud_tasks(clock)),
            push_cadence,
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

fn moment_from_hour(clock: Clock, minutes: i64) -> AgendaMoment {
    AgendaMoment::Floating(whole_hour(clock) + TimeDelta::minutes(minutes))
}

fn calendar_object_href(summary: &str) -> String {
    format!("{PERSONAL_CALENDAR}{summary}.ics")
}

fn nextcloud_provider() -> CaldavProvider {
    CaldavProvider::detected(NEXTCLOUD_SERVER, [PERSONAL_CALENDAR])
}

fn nextcloud_events(clock: Clock) -> UpcomingEventList {
    let timed_event = |summary: &str, start: i64, end: i64, location: &str| UpcomingEvent {
        summary: summary.to_owned(),
        calendar: "Personal".to_owned(),
        origin: EventOrigin::CalendarObject {
            object_href: calendar_object_href(summary),
            recurrence_id: moment_from_hour(clock, start),
        },
        start: moment_from_hour(clock, start),
        end: moment_from_hour(clock, end),
        location: location.to_owned(),
    };
    UpcomingEventList {
        events: vec![
            timed_event("Standup", -15, 15, ""),
            timed_event("Design review", 120, 180, "Room 2.14"),
            timed_event("Book club", 480, 600, "City library"),
        ],
        calendar_count: 1,
        failed_subscription_names: Vec::new(),
        provider: nextcloud_provider(),
    }
}

fn nextcloud_tasks(clock: Clock) -> OpenTaskList {
    let open_task =
        |summary: &str, deadline: TaskDeadline, priority: u32, progress: TaskProgress| OpenTask {
            summary: summary.to_owned(),
            calendar: "Tasks".to_owned(),
            object_href: calendar_object_href(summary),
            deadline,
            priority: TaskPriority::Ranked(priority),
            progress,
        };
    let due_in_minutes = |minutes: i64| TaskDeadline::Due(moment_from_hour(clock, minutes));
    OpenTaskList {
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
        task_list_count: 1,
        provider: nextcloud_provider(),
    }
}
