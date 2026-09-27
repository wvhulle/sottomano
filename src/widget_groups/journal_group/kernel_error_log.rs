use chrono::{DateTime, Local};

use crate::{
    data::source::{
        journal::KernelSeverity,
        kernel_errors::{KernelErrors, KernelOccurrence},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Level},
        font_awesome::Brands,
        tooltip_body::{Cell, TableRow, TooltipBody, row, section},
    },
};

const MESSAGE_CAP: usize = 200;
const TOOLTIP_ROWS: usize = 15;

impl KernelSeverity {
    fn level(self) -> Level {
        match self {
            KernelSeverity::Critical | KernelSeverity::Error => Level::Critical,
            KernelSeverity::Warning => Level::Warning,
        }
    }

    fn counts_toward_badge(self) -> bool {
        self != KernelSeverity::Warning
    }
}

impl TableRow for KernelOccurrence {
    const HEADINGS: &'static [&'static str] = &[];

    fn key(&self) -> String {
        DateTime::<Local>::from(self.latest)
            .format("%H:%M:%S")
            .to_string()
    }

    fn cells(&self) -> Vec<Cell> {
        let message: String = self.message.chars().take(MESSAGE_CAP).collect();
        let text = match self.count {
            1 => message,
            count => format!("{message} ×{count}"),
        };
        vec![text.into()]
    }

    fn level(&self) -> Level {
        self.severity.level()
    }
}

impl BarWidget for KernelErrors {
    fn icon(&self) -> Glyph {
        Brands::Linux.into()
    }

    fn badge(&self) -> Option<u32> {
        let distinct = self
            .occurrences()
            .iter()
            .filter(|occurrence| occurrence.severity.counts_toward_badge())
            .count();
        u32::try_from(distinct).ok().filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        self.entries
            .iter()
            .map(|entry| entry.severity.level())
            .max()
            .unwrap_or_default()
    }

    fn tooltip(&self) -> TooltipBody {
        let chronological = self.occurrences();
        let hidden = chronological.len().saturating_sub(TOOLTIP_ROWS);
        let hidden_notice = (hidden > 0).then(|| row("", format!("{hidden} earlier not shown")));
        let latest = chronological[hidden..].iter().map(TableRow::row);
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
    use crate::{data::source::journal::KernelEntry, widgets::tooltip_body::SectionBody};

    #[test]
    fn tooltip_lists_latest_messages_oldest_first_after_a_hidden_notice() {
        let errors = KernelErrors {
            entries: (0..17)
                .rev()
                .map(|secs| KernelEntry {
                    at: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
                    message: format!("message {secs}"),
                    severity: KernelSeverity::Error,
                })
                .collect(),
        };
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
