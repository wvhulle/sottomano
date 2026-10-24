use std::cmp::Ordering;

use chrono::{DateTime, Local};
use fast_dav_rs::caldav::{CalendarQueryFilter, PropFilter};
use icalendar::{Calendar, Component, Todo, TodoStatus};

use crate::caldav_tasks_events::{
    caldav_agenda::{AgendaMoment, Error, parsed_calendar_data},
    caldav_calendar_discovery::{CalendarCollection, CalendarHome},
};

const LOWEST_PRIORITY: u32 = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTask {
    pub summary: String,
    pub calendar: String,
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
    pub fn more_urgent_first(&self, other: &Self) -> Ordering {
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

fn open_tasks(calendars: &[Calendar], calendar_name: &str) -> Vec<OpenTask> {
    calendars
        .iter()
        .flat_map(Calendar::todos)
        .filter(|todo| still_open(todo))
        .map(|todo| OpenTask {
            summary: todo.get_summary().unwrap_or("(untitled)").trim().to_owned(),
            calendar: calendar_name.to_owned(),
            due: todo.get_due().and_then(AgendaMoment::of),
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

    use super::*;

    fn parsed() -> Vec<OpenTask> {
        let task_list = include_str!("caldav_fixtures/mixed_task_list.ics");
        open_tasks(&[Calendar::from_str(task_list).unwrap()], "Tasks")
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
        let brussels_two_pm = Brussels
            .with_ymd_and_hms(2026, 10, 30, 14, 0, 0)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(parsed()[2].due, Some(AgendaMoment::Timed(brussels_two_pm)));
    }

    #[test]
    fn dated_tasks_come_before_undated_ones() {
        let mut tasks = parsed();
        tasks.sort_by(OpenTask::more_urgent_first);
        let order: Vec<_> = tasks.iter().map(|task| task.summary.as_str()).collect();
        assert_eq!(order, ["File taxes", "Call bank", "Water plants"]);
    }
}
