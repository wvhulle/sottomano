use std::cmp::Ordering;

use chrono::{DateTime, Local};
use fast_dav_rs::caldav::{CalendarQueryFilter, PropFilter};
use icalendar::{Component, Todo, TodoStatus};

use crate::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment,
    caldav_agenda::{CalendarResource, Error, parsed_calendar_data},
    caldav_calendar_discovery::{CalendarCollection, CalendarHome},
};

const LOWEST_PRIORITY: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityBand {
    High,
    Medium,
    Low,
}

impl PriorityBand {
    #[must_use]
    pub fn of_ical_priority(priority: u32) -> Option<Self> {
        match priority {
            1..=4 => Some(PriorityBand::High),
            5 => Some(PriorityBand::Medium),
            6..=LOWEST_PRIORITY => Some(PriorityBand::Low),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            PriorityBand::High => "high",
            PriorityBand::Medium => "medium",
            PriorityBand::Low => "low",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTask {
    pub summary: String,
    pub calendar: String,
    pub object_href: String,
    pub due: Option<AgendaMoment>,
    pub priority: Option<u32>,
    pub percent_complete: Option<u8>,
    pub started: bool,
}

impl OpenTask {
    #[must_use]
    pub fn is_overdue(&self, now: DateTime<Local>) -> bool {
        self.due.is_some_and(|due| due.ends_before(now))
    }

    #[must_use]
    pub fn priority_band(&self) -> Option<PriorityBand> {
        self.priority.and_then(PriorityBand::of_ical_priority)
    }

    #[must_use]
    pub fn earliest_due_first(&self, other: &Self) -> Ordering {
        let undated_last = |task: &Self| task.due.map(AgendaMoment::starts_at);
        match (undated_last(self), undated_last(other)) {
            (Some(due), Some(other_due)) => due.cmp(&other_due),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| self.priority_or_lowest().cmp(&other.priority_or_lowest()))
    }

    fn priority_or_lowest(&self) -> u32 {
        self.priority.unwrap_or(LOWEST_PRIORITY + 1)
    }
}

pub async fn query_open_tasks(
    home: &CalendarHome,
    calendar: &CalendarCollection,
) -> Result<Vec<OpenTask>, Error> {
    let uncompleted = CalendarQueryFilter::new("VTODO")
        .with_prop_filters(vec![PropFilter::not_defined("COMPLETED")]);
    let objects = home
        .client
        .calendar_query(&calendar.href, &uncompleted, true)
        .await?;
    Ok(open_tasks(&parsed_calendar_data(objects), &calendar.name))
}

fn open_tasks(resources: &[CalendarResource], calendar_name: &str) -> Vec<OpenTask> {
    resources
        .iter()
        .flat_map(|resource| resource.calendar.todos().map(move |todo| (resource, todo)))
        .filter(|(_, todo)| still_open(todo))
        .map(|(resource, todo)| OpenTask {
            summary: todo.get_summary().unwrap_or("(untitled)").trim().to_owned(),
            calendar: calendar_name.to_owned(),
            object_href: resource.href.clone(),
            due: todo.get_due().map(AgendaMoment::of),
            priority: todo
                .get_priority()
                .filter(|priority| (1..=LOWEST_PRIORITY).contains(priority)),
            percent_complete: todo.get_percent_complete().filter(|percent| *percent > 0),
            started: todo.get_status() == Some(TodoStatus::InProcess),
        })
        .collect()
}

fn still_open(todo: &Todo) -> bool {
    todo.get_completed().is_none()
        && !matches!(
            todo.get_status(),
            Some(TodoStatus::Completed | TodoStatus::Cancelled)
        )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::{NaiveDate, TimeDelta, TimeZone};
    use chrono_tz::Europe::Brussels;
    use icalendar::Calendar;

    use super::*;

    fn parsed() -> Vec<OpenTask> {
        let task_list = include_str!("caldav_fixtures/mixed_task_list.ics");
        let resource = CalendarResource {
            href: "/remote.php/dav/calendars/wvhulle/tasks/list.ics".to_owned(),
            calendar: Calendar::from_str(task_list).unwrap(),
        };
        open_tasks(&[resource], "Tasks")
    }

    #[test]
    fn cancelled_tasks_are_not_open() {
        let summaries: Vec<_> = parsed().into_iter().map(|task| task.summary).collect();
        assert_eq!(summaries, ["File taxes", "Water plants", "Call bank"]);
    }

    #[test]
    fn a_date_due_task_is_overdue_from_the_next_day() {
        let taxes = &parsed()[0];
        let due_day = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let evening = Local
            .from_local_datetime(&due_day.and_hms_opt(22, 0, 0).unwrap())
            .unwrap();
        assert!(!taxes.is_overdue(evening));
        assert!(taxes.is_overdue(evening + TimeDelta::days(1)));
        assert_eq!(taxes.priority, Some(1));
    }

    #[test]
    fn progress_and_undefined_priority_are_read() {
        let tasks = parsed();
        assert!(tasks[1].started);
        assert_eq!(tasks[1].percent_complete, Some(40));
        assert_eq!(tasks[1].due, None);
        assert_eq!(tasks[2].priority, None);
    }

    #[test]
    fn a_zoned_due_time_converts_to_local_time() {
        let brussels_two_pm = Brussels.with_ymd_and_hms(2026, 10, 30, 14, 0, 0).unwrap();
        assert_eq!(parsed()[2].due, Some(AgendaMoment::Zoned(brussels_two_pm)));
    }

    #[test]
    fn the_earliest_deadline_comes_first_and_undated_tasks_last() {
        let mut tasks = parsed();
        tasks.sort_by(OpenTask::earliest_due_first);
        let order: Vec<_> = tasks.iter().map(|task| task.summary.as_str()).collect();
        assert_eq!(order, ["File taxes", "Call bank", "Water plants"]);
    }

    #[test]
    fn priorities_fall_into_the_rfc_5545_bands() {
        let tasks = parsed();
        assert_eq!(tasks[0].priority_band(), Some(PriorityBand::High));
        assert_eq!(tasks[2].priority_band(), None);
        assert_eq!(
            PriorityBand::of_ical_priority(5),
            Some(PriorityBand::Medium)
        );
        assert_eq!(PriorityBand::of_ical_priority(7), Some(PriorityBand::Low));
    }
}
