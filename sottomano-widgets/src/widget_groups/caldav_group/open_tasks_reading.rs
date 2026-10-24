use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::caldav_tasks_events::{
    caldav_agenda::CaldavAgenda, caldav_task_query::OpenTask,
};

use crate::{
    clock::Clock,
    widget_groups::caldav_group::upcoming_events_reading::{change_delivery_caption, moment_cell},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

const LISTED_TASKS: usize = 12;

pub struct OpenTasks<'a>(pub &'a CaldavAgenda);

struct TaskRow<'a> {
    task: &'a OpenTask,
    now: DateTime<Local>,
}

impl TableRow<3> for TaskRow<'_> {
    fn table_record(self) -> TableRecord<3> {
        let record = TableRecord::new(
            self.task.summary.as_str(),
            [
                self.task.due.map(moment_cell),
                Some(Cell::Hint(progress_hint(self.task))),
                Some(Cell::Hint(self.task.calendar.clone())),
            ],
        );
        if self.task.is_overdue(self.now) {
            record.level(Severity::Warning)
        } else {
            record
        }
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
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Due"),
                ColumnHeading::new("Progress"),
                ColumnHeading::new("List").align_x(Horizontal::Left),
            ],
            self.0
                .tasks
                .iter()
                .take(LISTED_TASKS)
                .map(|task| TaskRow { task, now }),
        )
        .titled((
            "Open tasks",
            format!(
                "Tasks without a completion date, overdue and soonest due first; {}",
                change_delivery_caption(self.0)
            ),
        ))])
    }
}
