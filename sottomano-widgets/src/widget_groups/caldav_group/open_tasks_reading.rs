use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::caldav_tasks_events::{
    caldav_agenda::CaldavAgenda,
    caldav_server_provider::CaldavProvider,
    caldav_task_query::{OpenTask, PriorityBand, TaskDeadline, TaskProgress},
    nextcloud_web_links::NextcloudWebRoot,
};

use crate::{
    clock::Clock,
    widget_groups::caldav_group::upcoming_events_reading::{
        calendar_count_caption, linked_title, moment_cell, nextcloud_footer,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow, TooltipBody,
        },
        widget_state::WidgetState,
    },
};

const LISTED_TASKS: usize = 16;

pub struct OpenTasks<'a>(pub &'a CaldavAgenda);

struct TaskRow<'a> {
    task: &'a OpenTask,
    now: DateTime<Local>,
    provider: &'a CaldavProvider,
}

impl TableRow<4> for TaskRow<'_> {
    fn table_record(self) -> TableRecord<4> {
        let details_link = self
            .provider
            .nextcloud_web_root()
            .and_then(|nextcloud| nextcloud.task_details(self.task));
        let title = match details_link {
            Some(url) => linked_title(&self.task.summary, url),
            None => self.task.summary.as_str().into(),
        };
        TableRecord::new(
            title,
            [
                self.task.deadline.due_moment().map(moment_cell),
                self.task
                    .priority
                    .band()
                    .map(|band| Cell::Hint(band.name().to_owned())),
                Some(Cell::Hint(progress_hint(self.task))),
                Some(Cell::Hint(self.task.calendar.clone())),
            ],
        )
        .level(task_severity(self.task, self.now))
    }
}

fn task_severity(task: &OpenTask, now: DateTime<Local>) -> Severity {
    if task.is_overdue(now) {
        return Severity::Critical;
    }
    match task.priority.band() {
        Some(PriorityBand::High) => Severity::Warning,
        Some(PriorityBand::Low) => Severity::Low,
        Some(PriorityBand::Medium) | None => Severity::Normal,
    }
}

fn progress_hint(task: &OpenTask) -> String {
    match task.progress {
        TaskProgress::PercentComplete(percent) => format!("{percent} %"),
        TaskProgress::Started => "started".to_owned(),
        TaskProgress::NotStarted => String::new(),
    }
}

impl BarWidget for OpenTasks<'_> {
    fn name(&self) -> &'static str {
        "Open tasks"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ListChecks(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        let overdue = self.0.overdue_task_count(clock.local());
        Some(match overdue {
            0 => self.0.tasks.len().to_string(),
            _ => format!("{} ({overdue} late)", self.0.tasks.len()),
        })
    }

    fn state(&self, clock: Clock) -> WidgetState {
        if self.0.overdue_task_count(clock.local()) > 0 {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn visible_in_bar(&self) -> bool {
        !self.0.tasks.is_empty()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let now = clock.local();
        let provider = &self.0.provider;
        let with_deadline: Vec<&OpenTask> = self
            .0
            .tasks
            .iter()
            .filter(|task| matches!(task.deadline, TaskDeadline::Due(_)))
            .collect();
        let without_deadline = self.0.tasks.len() - with_deadline.len();
        let hidden = with_deadline.len().saturating_sub(LISTED_TASKS);
        let rows = with_deadline
            .into_iter()
            .take(LISTED_TASKS)
            .map(|task| TaskRow {
                task,
                now,
                provider,
            });
        TooltipBody::sections([
            Section::column_table(
                [
                    ColumnHeading::titled("Due"),
                    ColumnHeading::titled("Priority"),
                    ColumnHeading::titled("Progress"),
                    ColumnHeading::titled("List").align_x(Horizontal::Left),
                ],
                rows,
            )
            .titled((
                "Open tasks",
                format!(
                    "Open tasks with a deadline from all lists, earliest first. Overdue in red, \
                     high priority in yellow, low priority dimmed; {}",
                    calendar_count_caption(self.0)
                ),
            )),
            nextcloud_footer("open Tasks", provider, NextcloudWebRoot::tasks_app, hidden)
                .with_entries(
                    (without_deadline > 0).then(|| {
                        KeyValue::new("Without deadline", Quantity::count(without_deadline))
                    }),
                ),
        ])
    }
}
