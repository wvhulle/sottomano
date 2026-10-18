use iced::alignment::Horizontal;
use vetrata_config::Config;
use vetrata_sources::previous_shutdown::{JobResult, LastShutdown, StopOutcome, UnitStop};

use crate::{
    widget_groups::{
        systemd_manager_group::{SystemdManagerMessage, SystemdTarget, elapsed_text},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::{ConfiguredGroupItem, GroupItem},
        phosphor::Phosphor,
        tooltip_body::{ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

impl TableRow<2> for UnitStop {
    fn table_record(&self) -> TableRecord<2> {
        let cells = match &self.outcome {
            StopOutcome::Unfinished => [Some("did not finish".into()), None],
            StopOutcome::Finished {
                duration,
                job_result,
                ..
            } => [
                Some(elapsed_text(*duration).into()),
                (*job_result != JobResult::Done).then(|| job_result.to_string().into()),
            ],
        };
        TableRecord::new(self.unit.name(), cells).level(self.outcome.severity())
    }
}

impl BarWidget for LastShutdown {
    fn name(&self) -> &'static str {
        "Last shutdown"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Power(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(elapsed_text(self.duration))
    }

    fn state(&self) -> WidgetState {
        self.slowest_stops
            .iter()
            .map(|stop| stop.outcome.severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Took"),
                ColumnHeading::new("Result").align_x(Horizontal::Left),
            ],
            self.slowest_stops.iter().map(UnitStop::table_record),
        )
        .titled((
            "Slowest stops at last shutdown",
            "From the previous boot's journal: shutdown start to its last entry. Amber stops \
             crossed slow_shutdown_stop_secs or never finished.",
        ))])
    }
}

impl ConfiguredGroupItem<SystemdManagerMessage> for LastShutdown {
    fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem::reading(self, SystemdTarget::LastShutdown).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            SystemdManagerMessage::OpenLog(TerminalCommand::journal(
                config,
                self.follow_log_args(),
            )),
        )
    }
}
