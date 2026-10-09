use crate::{
    data::{
        severity::Severity,
        source::coredumps::{CoredumpEntry, Coredumps},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, table_section},
        widget_state::WidgetState,
    },
};

impl TableRow for CoredumpEntry {
    const HEADINGS: &'static [&'static str] = &["Process", "Signal", "PID", "Unit"];

    fn key(&self) -> Cell {
        Cell::Time {
            at: self.at.into(),
            part: TimePart::TimeOfDay,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            self.command.clone().into(),
            self.signal.clone().into(),
            self.pid.clone().into(),
            self.unit
                .as_ref()
                .map_or(Cell::Missing, |unit| unit.name().into()),
        ]
    }

    fn level(&self) -> Severity {
        self.severity
    }
}

impl Coredumps {
    pub fn count_row(&self) -> Row {
        row("Coredumps", self.entries.len().to_string())
    }
}

impl BarWidget for Coredumps {
    fn icon(&self) -> Glyph {
        Phosphor::Bug(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.entries.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.entries
            .iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![table_section(
            "Coredumps",
            self.entries.iter().cloned(),
        )])
    }
}
