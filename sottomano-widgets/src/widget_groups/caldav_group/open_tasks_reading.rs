use std::num::NonZeroUsize;

use chrono::{DateTime, Local, Locale, NaiveDate};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::caldav_tasks_events::{
    caldav_open_tasks::OpenTaskList,
    caldav_server_provider::CaldavProvider,
    caldav_task_query::{OpenTask, PriorityBand, TaskDeadline, TaskDueStatus, TaskProgress},
    nextcloud_web_links::NextcloudWebRoot,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::caldav_group::upcoming_events_reading::{
        event_date_cell, event_time_cell, linked_title, nextcloud_footer,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
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
                progress_cell(self.task),
                ValueCell::Hint(self.task.calendar.clone()),
            ],
        )
        .level(task_severity(self.task, self.now))
    }
}

fn task_severity(task: &OpenTask, now: DateTime<Local>) -> Severity {
    match (task.due_status(now), task.priority.band()) {
        (TaskDueStatus::Overdue, _) => Severity::Critical,
        (TaskDueStatus::NotYetDue, Some(PriorityBand::High)) => Severity::Warning,
        (TaskDueStatus::NotYetDue, Some(PriorityBand::Low)) => Severity::Low,
        (TaskDueStatus::NotYetDue, Some(PriorityBand::Medium) | None) => Severity::Normal,
    }
}

fn progress_cell(task: &OpenTask) -> ValueCell {
    match task.progress {
        TaskProgress::PercentComplete(percent) => ValueCell::Hint(format!("{percent} %")),
        TaskProgress::Started => ValueCell::Hint("started".to_owned()),
        TaskProgress::NotStarted => ValueCell::Blank,
    }
}

impl GroupItemReading for OpenTaskList {
    fn name(&self) -> &'static str {
        "Open tasks"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ListChecks(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        let overdue_count = self.overdue_task_count(clock.local());
        (overdue_count > 0).then(|| overdue_count.to_string())
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        if self.overdue_task_count(clock.local()) > 0 {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn visible_on_panel(&self) -> bool {
        !self.tasks.is_empty()
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
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
        HoverCardBody::sections([
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
            .without_blank_columns()
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
