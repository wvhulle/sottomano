use crate::{
    data::{
        severity::Severity,
        source::previous_shutdown::{LastShutdown, StopOutcome, UnitStop},
    },
    widget_groups::systemd_manager_group::elapsed_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Cell, TableRow, TooltipBody, table_section},
        widget_state::WidgetState,
    },
};

const COMPLETED_JOB_RESULT: &str = "done";

impl StopOutcome {
    fn level(&self) -> Severity {
        match self {
            StopOutcome::Unfinished | StopOutcome::Finished { slow: true, .. } => Severity::Warning,
            StopOutcome::Finished { slow: false, .. } => Severity::Normal,
        }
    }
}

impl TableRow for UnitStop {
    const HEADINGS: &'static [&'static str] = &["Took", "Result"];

    fn key(&self) -> Cell {
        self.unit.name().into()
    }

    fn cells(&self) -> Vec<Cell> {
        match &self.outcome {
            StopOutcome::Unfinished => vec!["did not finish".into(), Cell::Missing],
            StopOutcome::Finished {
                duration,
                job_result,
                ..
            } => vec![
                elapsed_text(*duration).into(),
                if job_result == COMPLETED_JOB_RESULT {
                    Cell::Missing
                } else {
                    job_result.clone().into()
                },
            ],
        }
    }

    fn level(&self) -> Severity {
        self.outcome.level()
    }
}

impl BarWidget for LastShutdown {
    fn icon(&self) -> Glyph {
        Phosphor::Power(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(elapsed_text(self.duration))
    }

    fn state(&self) -> WidgetState {
        self.slowest_stops
            .iter()
            .map(|stop| stop.outcome.level())
            .max()
            .filter(|level| *level > Severity::Normal)
            .map_or(WidgetState::Idle, WidgetState::from)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![table_section(
            "Slowest stops at last shutdown",
            self.slowest_stops.iter().cloned(),
        )])
    }
}
