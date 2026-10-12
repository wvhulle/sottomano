use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::previous_shutdown::{LastShutdown, StopOutcome, UnitStop},
    },
    widget_groups::{
        systemd_manager_group::{SystemdManagerMessage, SystemdTarget, elapsed_text},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        phosphor::Phosphor,
        tooltip_body::{Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

const COMPLETED_JOB_RESULT: &str = "done";

impl StopOutcome {
    fn severity(&self) -> Severity {
        match self {
            StopOutcome::Unfinished => Severity::Warning,
            StopOutcome::Finished { severity, .. } => *severity,
        }
    }
}

impl UnitStop {
    fn table_record(&self) -> TableRecord<2> {
        let cells = match &self.outcome {
            StopOutcome::Unfinished => [Some("did not finish".into()), None],
            StopOutcome::Finished {
                duration,
                job_result,
                ..
            } => [
                Some(elapsed_text(*duration).into()),
                (job_result != COMPLETED_JOB_RESULT).then(|| job_result.clone().into()),
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
            ["Took", "Result"],
            self.slowest_stops.iter().map(UnitStop::table_record),
        )
        .titled((
            "Slowest stops at last shutdown",
            "From the previous boot's journal: shutdown start to its last entry. Amber stops \
             crossed slow_shutdown_stop_secs or never finished.",
        ))])
    }
}

impl LastShutdown {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem::reading(self, SystemdTarget::LastShutdown).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            SystemdManagerMessage::OpenLog(TerminalCommand::journal(
                config,
                self.follow_log_args(),
            )),
        )
    }
}
