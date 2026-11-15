use std::num::NonZeroUsize;

use chrono::{DateTime, Local, NaiveDate};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    caldav_tasks_events::{
        caldav_open_tasks::OpenTaskList,
        caldav_server_provider::CaldavProvider,
        caldav_task_query::{OpenTask, PriorityBand, TaskDeadline, TaskDueStatus, TaskProgress},
        nextcloud_web_links::NextcloudWebRoot,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_grouped_table},
    widget_groups::caldav_group::upcoming_events_reading::{
        event_date_cell, event_time_cell, linked_title, nextcloud_app_link,
    },
    widgets::{
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
        TableRecord::with_cells(
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
    match task.due_status(now) {
        TaskDueStatus::Overdue => Severity::Critical,
        TaskDueStatus::NotYetDue => match task.priority.band() {
            Some(PriorityBand::High) => Severity::Warning,
            Some(PriorityBand::Low) => Severity::Low,
            Some(PriorityBand::Medium) | None => Severity::Normal,
        },
    }
}

fn progress_cell(task: &OpenTask) -> ValueCell {
    match task.progress {
        TaskProgress::PercentComplete(percent) => ValueCell::Hint(format!("{percent} %")),
        TaskProgress::Started => ValueCell::Hint("started".to_owned()),
        TaskProgress::NotStarted => ValueCell::Blank,
    }
}

pub(crate) fn open_tasks_reading_row(open_tasks: &OpenTaskList, clock: Clock) -> ReadingRow<'_> {
    let overdue_count = open_tasks.overdue_task_count(clock.local());
    let (label, state) = if overdue_count > 0 {
        (
            ReadoutLabel::Labeled(overdue_count.to_string()),
            ReadingDisplayState::Warning,
        )
    } else {
        (ReadoutLabel::Unlabeled, ReadingDisplayState::Idle)
    };
    ReadingRow {
        readout: RowReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        panel_visibility: if open_tasks.tasks.is_empty() {
            PanelVisibility::Hidden
        } else {
            PanelVisibility::Shown
        },
        ..ReadingRow::glyph_only(
            OpenTaskList::NAME,
            Phosphor::ListChecks(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| open_tasks_hover_card(open_tasks, card_clock)),
        )
    }
}

pub(crate) fn open_tasks_hover_card(open_tasks: &OpenTaskList, clock: Clock) -> Vec<Section> {
    let now = clock.local();
    let provider = &open_tasks.provider;
    let with_deadline: Vec<&OpenTask> = open_tasks
        .tasks
        .iter()
        .filter(|task| matches!(task.deadline, TaskDeadline::Due(_)))
        .collect();
    let without_deadline = open_tasks.tasks.len() - with_deadline.len();
    let rows: Vec<_> = with_deadline
        .into_iter()
        .map(|task| TaskRow {
            task,
            now,
            provider,
        })
        .collect();
    vec![
        Section::new(
            SectionTitle::captioned(
                "Deadlines",
                format!(
                    "Open tasks with a deadline from all lists, earliest first. Overdue in red, \
                     high priority in yellow, low priority dimmed; {} task lists over CalDAV.",
                    open_tasks.task_list_count
                ),
            ),
            headed_grouped_table(
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
            .without_blank_columns(ValueCell::is_blank),
        ),
        Section::new(
            "Other tasks",
            dictionary_table(
                (without_deadline > 0)
                    .then(|| {
                        LabeledValue::new("Without deadline", Quantity::count(without_deadline))
                    })
                    .into_iter()
                    .chain(nextcloud_app_link(
                        "open Tasks",
                        provider,
                        NextcloudWebRoot::tasks_app,
                    )),
            ),
        ),
    ]
}
