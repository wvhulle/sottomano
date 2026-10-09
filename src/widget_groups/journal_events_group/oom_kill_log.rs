use crate::{
    data::{
        severity::Severity,
        source::oom_kills::{OomKillEntry, OomKiller, OomKills},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, table_section},
        widget_state::WidgetState,
    },
};

impl OomKiller {
    fn label(self) -> &'static str {
        match self {
            OomKiller::Kernel => "kernel",
            OomKiller::Oomd => "systemd-oomd",
        }
    }
}

impl TableRow for OomKillEntry {
    const HEADINGS: &'static [&'static str] = &["Unit", "Killed by"];

    fn key(&self) -> Cell {
        Cell::Time {
            at: self.at.into(),
            part: TimePart::TimeOfDay,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        vec![self.unit.name().into(), self.killer.label().into()]
    }

    fn level(&self) -> Severity {
        Severity::Critical
    }
}

impl OomKills {
    pub fn count_row(&self) -> Row {
        row("OOM kills", self.entries.len().to_string())
    }
}

impl BarWidget for OomKills {
    fn icon(&self) -> Glyph {
        Phosphor::Memory(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.entries.len()).ok()
    }

    fn state(&self) -> WidgetState {
        Severity::Critical.into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![table_section(
            "OOM kills",
            self.entries.iter().cloned(),
        )])
    }
}
