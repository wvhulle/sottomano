use std::cmp::Ordering;

use chrono::{DateTime, Local};
use fast_dav_rs::caldav::{CalendarQueryFilter, PropFilter};
use icalendar::{Component, Todo, TodoStatus};

use crate::caldav_tasks_events::{
    agenda_moment_zones::{AgendaMoment, DeadlinePassage},
    caldav_agenda::{CalendarResource, Error, parsed_calendar_data},
    caldav_calendar_discovery::{CalendarCollection, CalendarHome},
    caldav_event_query::displayed_summary,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDeadline {
    Due(AgendaMoment),
    Undated,
}

impl TaskDeadline {
    #[must_use]
    pub fn due_moment(self) -> Option<AgendaMoment> {
        match self {
            TaskDeadline::Due(due) => Some(due),
            TaskDeadline::Undated => None,
        }
    }

    fn earliest_first(self, other: Self) -> Ordering {
        match (self, other) {
            (TaskDeadline::Due(due), TaskDeadline::Due(other_due)) => {
                due.starts_at().cmp(&other_due.starts_at())
            }
            (TaskDeadline::Due(_), TaskDeadline::Undated) => Ordering::Less,
            (TaskDeadline::Undated, TaskDeadline::Due(_)) => Ordering::Greater,
            (TaskDeadline::Undated, TaskDeadline::Undated) => Ordering::Equal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Ranked(u32),
    Unranked,
}

impl TaskPriority {
    #[must_use]
    pub fn band(self) -> Option<PriorityBand> {
        match self {
            TaskPriority::Ranked(priority) => PriorityBand::of_ical_priority(priority),
            TaskPriority::Unranked => None,
        }
    }
}

impl From<u32> for TaskPriority {
    fn from(priority: u32) -> Self {
        if (1..=LOWEST_PRIORITY).contains(&priority) {
            TaskPriority::Ranked(priority)
        } else {
            TaskPriority::Unranked
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskProgress {
    NotStarted,
    Started,
    PercentComplete(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTask {
    pub summary: String,
    pub calendar: String,
    pub object_href: String,
    pub deadline: TaskDeadline,
    pub priority: TaskPriority,
    pub progress: TaskProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDueStatus {
    Overdue,
    NotYetDue,
}

impl OpenTask {
    #[must_use]
    pub fn due_status(&self, now: DateTime<Local>) -> TaskDueStatus {
        match self.deadline {
            TaskDeadline::Due(due) if due.passage_at(now) == DeadlinePassage::Passed => {
                TaskDueStatus::Overdue
            }
            _ => TaskDueStatus::NotYetDue,
        }
    }

    #[must_use]
    pub fn earliest_due_first(&self, other: &Self) -> Ordering {
        self.deadline
            .earliest_first(other.deadline)
            .then_with(|| self.priority.cmp(&other.priority))
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
        .flat_map(|resource| {
            resource
                .calendar
                .todos()
                .filter(|todo| {
                    todo.get_completed().is_none()
                        && !matches!(
                            todo.get_status(),
                            Some(TodoStatus::Completed | TodoStatus::Cancelled)
                        )
                })
                .map(move |todo| open_task(&resource.href, todo, calendar_name))
        })
        .collect()
}

fn open_task(object_href: &str, todo: &Todo, calendar_name: &str) -> OpenTask {
    OpenTask {
        summary: displayed_summary(todo.get_summary().unwrap_or_default()),
        calendar: calendar_name.to_owned(),
        object_href: object_href.to_owned(),
        deadline: todo.get_due().map_or(TaskDeadline::Undated, |due| {
            TaskDeadline::Due(AgendaMoment::of(due))
        }),
        priority: todo
            .get_priority()
            .map_or(TaskPriority::Unranked, TaskPriority::from),
        progress: task_progress(todo),
    }
}

fn task_progress(todo: &Todo) -> TaskProgress {
    match todo.get_percent_complete() {
        Some(percent) if percent > 0 => TaskProgress::PercentComplete(percent),
        _ if todo.get_status() == Some(TodoStatus::InProcess) => TaskProgress::Started,
        _ => TaskProgress::NotStarted,
    }
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
        assert_eq!(taxes.due_status(evening), TaskDueStatus::NotYetDue);
        assert_eq!(
            taxes.due_status(evening + TimeDelta::days(1)),
            TaskDueStatus::Overdue
        );
        assert_eq!(taxes.priority, TaskPriority::Ranked(1));
    }

    #[test]
    fn progress_and_undefined_priority_are_read() {
        let tasks = parsed();
        assert_eq!(tasks[1].progress, TaskProgress::PercentComplete(40));
        assert_eq!(tasks[1].deadline, TaskDeadline::Undated);
        assert_eq!(tasks[2].priority, TaskPriority::Unranked);
    }

    #[test]
    fn a_zoned_due_time_converts_to_local_time() {
        let brussels_two_pm = Brussels.with_ymd_and_hms(2026, 10, 30, 14, 0, 0).unwrap();
        assert_eq!(
            parsed()[2].deadline,
            TaskDeadline::Due(AgendaMoment::Zoned(brussels_two_pm))
        );
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
        assert_eq!(tasks[0].priority.band(), Some(PriorityBand::High));
        assert_eq!(tasks[2].priority.band(), None);
        assert_eq!(
            PriorityBand::of_ical_priority(5),
            Some(PriorityBand::Medium)
        );
        assert_eq!(PriorityBand::of_ical_priority(7), Some(PriorityBand::Low));
    }
}
