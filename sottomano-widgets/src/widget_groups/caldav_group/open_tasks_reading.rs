use std::num::NonZeroUsize;

use chrono::{DateTime, Local, Locale, NaiveDate};
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::caldav_tasks_events::{
    caldav_open_tasks::OpenTaskList,
    caldav_server_provider::CaldavProvider,
    caldav_task_query::{OpenTask, PriorityBand, TaskDeadline, TaskProgress},
    nextcloud_web_links::NextcloudWebRoot,
};

use crate::{
    clock::Clock,
    widget_groups::caldav_group::upcoming_events_reading::{
        event_date_cell, event_time_cell, linked_title, nextcloud_footer,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
        widget_state::WidgetState,
    },
};

const LISTED_TASKS: NonZeroUsize = NonZeroUsize::new(16).unwrap();

struct TaskRow<'a> {
    task: &'a OpenTask,
    now: DateTime<Local>,
    provider: &'a CaldavProvider,
}

impl TaskRow<'_> {
    fn due_on(&self) -> Option<NaiveDate> {
        self.task
            .deadline
            .due_moment()
            .map(|due| due.starts_at().date_naive())
    }
}

impl IntoTableRecord<5> for &TaskRow<'_> {
    fn into_table_record(self) -> TableRecord<5> {
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
                self.task
                    .deadline
                    .due_moment()
                    .map_or(ValueCell::Blank, event_date_cell),
                self.task
                    .deadline
                    .due_moment()
                    .map_or(ValueCell::Blank, event_time_cell),
                self.task.priority.band().map_or(ValueCell::Blank, |band| {
                    ValueCell::Hint(band.name().to_owned())
                }),
                ValueCell::Hint(progress_hint(self.task)),
                ValueCell::Hint(self.task.calendar.clone()),
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

impl BarWidget for OpenTaskList {
    fn name(&self) -> &'static str {
        "Open tasks"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ListChecks(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        let overdue_count = self.overdue_task_count(clock.local());
        (overdue_count > 0).then(|| overdue_count.to_string())
    }

    fn state(&self, clock: Clock) -> WidgetState {
        if self.overdue_task_count(clock.local()) > 0 {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn visible_in_bar(&self) -> bool {
        !self.tasks.is_empty()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let now = clock.local();
        let provider = &self.provider;
        let with_deadline: Vec<&OpenTask> = self
            .tasks
            .iter()
            .filter(|task| matches!(task.deadline, TaskDeadline::Due(_)))
            .collect();
        let without_deadline = self.tasks.len() - with_deadline.len();
        let rows: Vec<_> = with_deadline
            .into_iter()
            .map(|task| TaskRow {
                task,
                now,
                provider,
            })
            .collect();
        TooltipBody::sections([
            Section::grouped_column_table(
                [
                    ColumnHeading::titled("Day"),
                    ColumnHeading::titled("Time"),
                    ColumnHeading::titled("Priority"),
                    ColumnHeading::titled("Progress"),
                    ColumnHeading::titled("List").align_x(Horizontal::Left),
                ],
                rows.chunk_by(|earlier, later| earlier.due_on() == later.due_on()),
            )
            .truncated_to(LISTED_TASKS)
            .titled((
                "Open tasks",
                format!(
                    "Open tasks with a deadline from all lists, earliest first. Overdue in red, \
                     high priority in yellow, low priority dimmed; {} task lists over CalDAV.",
                    self.task_list_count
                ),
            )),
            nextcloud_footer("open Tasks", provider, NextcloudWebRoot::tasks_app).with_entries(
                (without_deadline > 0).then(|| {
                    LabeledValue::new("Without deadline", Quantity::count(without_deadline))
                }),
            ),
        ])
    }
}
