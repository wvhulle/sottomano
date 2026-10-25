use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::caldav_tasks_events::{
    caldav_agenda::CaldavAgenda,
    caldav_task_query::{OpenTask, PriorityBand},
    nextcloud_web_links::NextcloudWebRoot,
};

use crate::{
    clock::Clock,
    widget_groups::caldav_group::upcoming_events_reading::{
        change_delivery_caption, linked_title, moment_cell, nextcloud_footer,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
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
    nextcloud: Option<&'a NextcloudWebRoot>,
}

impl TableRow<4> for TaskRow<'_> {
    fn table_record(self) -> TableRecord<4> {
        TableRecord::new(
            linked_title(
                &self.task.summary,
                self.nextcloud
                    .and_then(|nextcloud| nextcloud.task_details(self.task)),
            ),
            [
                self.task.due.map(moment_cell),
                self.task
                    .priority_band()
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
    match task.priority_band() {
        Some(PriorityBand::High) => Severity::Warning,
        Some(PriorityBand::Low) => Severity::Low,
        Some(PriorityBand::Medium) | None => Severity::Normal,
    }
}

fn progress_hint(task: &OpenTask) -> String {
    match (task.percent_complete, task.started) {
        (Some(percent), _) => format!("{percent} %"),
        (None, true) => "started".to_owned(),
        (None, false) => String::new(),
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
        let nextcloud = self.0.provider.nextcloud_web_root();
        let with_deadline: Vec<&OpenTask> = self
            .0
            .tasks
            .iter()
            .filter(|task| task.due.is_some())
            .collect();
        let without_deadline = self.0.tasks.len() - with_deadline.len();
        let hidden = with_deadline.len().saturating_sub(LISTED_TASKS);
        let rows = with_deadline
            .into_iter()
            .take(LISTED_TASKS)
            .map(|task| TaskRow {
                task,
                now,
                nextcloud,
            });
        TooltipBody::sections([
            Section::column_table(
                [
                    ColumnHeading::new("Due"),
                    ColumnHeading::new("Priority"),
                    ColumnHeading::new("Progress"),
                    ColumnHeading::new("List").align_x(Horizontal::Left),
                ],
                rows,
            )
            .titled((
                "Open tasks",
                format!(
                    "Open tasks with a deadline from all lists, earliest first. Overdue in red, \
                     high priority in yellow, low priority dimmed; {}",
                    change_delivery_caption(self.0)
                ),
            )),
            nextcloud_footer(
                "open Tasks",
                nextcloud.map(NextcloudWebRoot::tasks_app),
                hidden,
            )
            .with_entries(
                (without_deadline > 0)
                    .then(|| KeyValue::new("Without deadline", without_deadline.to_string())),
            ),
        ])
    }
}
