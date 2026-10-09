use crate::{
    data::{
        severity::Severity,
        source::kernel_errors::{KernelErrors, KernelOccurrence},
    },
    widget_groups::journal_events_group::MESSAGE_CAP,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, section},
        widget_state::WidgetState,
    },
};

const TOOLTIP_ROWS: usize = 15;

impl TableRow for KernelOccurrence {
    const HEADINGS: &'static [&'static str] = &[];

    fn key(&self) -> Cell {
        Cell::Time {
            at: self.latest.into(),
            part: TimePart::TimeOfDay,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        let message: String = self.message.chars().take(MESSAGE_CAP).collect();
        let text = match self.count {
            1 => message,
            count => format!("{message} ×{count}"),
        };
        vec![text.into()]
    }

    fn level(&self) -> Severity {
        self.severity
    }
}

impl KernelErrors {
    pub fn count_row(&self) -> Row {
        row("Kernel errors", self.oldest_first.len().to_string())
    }
}

impl BarWidget for KernelErrors {
    fn icon(&self) -> Glyph {
        Phosphor::LinuxLogo(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        let distinct = self
            .oldest_first
            .iter()
            .filter(|occurrence| occurrence.severity >= Severity::Warning)
            .count();
        u32::try_from(distinct).ok().filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        self.oldest_first
            .iter()
            .map(|occurrence| occurrence.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let hidden = self.oldest_first.len().saturating_sub(TOOLTIP_ROWS);
        let hidden_notice = (hidden > 0).then(|| row("", format!("{hidden} earlier not shown")));
        let latest = self.oldest_first[hidden..].iter().map(TableRow::row);
        TooltipBody::Sections(vec![section(
            "Kernel log",
            hidden_notice.into_iter().chain(latest).collect(),
        )])
    }
}

#[cfg(test)]
mod tests {
    use std::{
        iter::once,
        time::{Duration, SystemTime},
    };

    use super::*;
    use crate::{data::source::kernel_errors::KernelEntry, widgets::tooltip_body::SectionBody};

    #[test]
    fn tooltip_lists_latest_messages_oldest_first_after_a_hidden_notice() {
        let errors = KernelErrors::from_entries(
            (0..17)
                .rev()
                .map(|secs| KernelEntry {
                    at: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
                    message: format!("message {secs}"),
                    severity: Severity::Critical,
                })
                .collect(),
        )
        .expect("errors present");
        let TooltipBody::Sections(sections) = errors.tooltip() else {
            panic!("expected sections");
        };
        let SectionBody::Rows(rows) = &sections[0].body else {
            panic!("expected rows");
        };
        let values: Vec<Cell> = rows.iter().map(|row| row.cells[0].clone()).collect();
        let expected: Vec<Cell> = once("2 earlier not shown".to_string())
            .chain((2..17).map(|secs| format!("message {secs}")))
            .map(Cell::from)
            .collect();
        assert_eq!(values, expected);
    }
}
