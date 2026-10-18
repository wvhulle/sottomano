use iced::alignment::Horizontal;
use vetrata_config::{Config, severity::Severity};
use vetrata_sources::kernel_errors::KernelOccurrence;

use crate::{
    widget_groups::{
        journal_events_group::{
            JournalEventsGroup, JournalEventsMessage, JournalTarget, MESSAGE_CAP,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::LocalizedTimeFormat,
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

const TOOLTIP_ROWS: usize = 15;

impl JournalEventsGroup {
    pub(super) fn kernel_log_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsMessage>> {
        let kernel = self.kernel.display()?;
        Some(GroupItem::reading(kernel, JournalTarget::Kernel).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            JournalEventsMessage::OpenLog(TerminalCommand::journal(
                config,
                KernelOccurrence::follow_log_args(),
            )),
        ))
    }
}

impl TableRow<2> for &KernelOccurrence {
    fn table_record(self) -> TableRecord<2> {
        let message: String = self.message.chars().take(MESSAGE_CAP).collect();
        let at = Cell::Time {
            at: self.latest.into(),
            format: LocalizedTimeFormat::TimeOfDay,
        };
        TableRecord::new(
            at,
            [
                Some(Cell::Hint(self.count.to_string())),
                Some(message.into()),
            ],
        )
        .level(self.severity)
    }
}

impl BarWidget for Vec<KernelOccurrence> {
    fn name(&self) -> &'static str {
        "Kernel log"
    }

    fn icon(&self) -> Glyph {
        Phosphor::LinuxLogo(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        let distinct = self
            .iter()
            .filter(|occurrence| occurrence.severity >= Severity::Warning)
            .count();
        u32::try_from(distinct).ok().filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        self.iter()
            .map(|occurrence| occurrence.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let hidden = self.len().saturating_sub(TOOLTIP_ROWS);
        let hidden_notice = (hidden > 0).then(|| {
            TableRecord::new(
                "",
                [None, Some(format!("{hidden} earlier not shown").into())],
            )
        });
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Count"),
                ColumnHeading::new("Message").align_x(Horizontal::Left),
            ],
            hidden_notice
                .into_iter()
                .chain(self[hidden..].iter().map(TableRow::table_record)),
        )
        .titled((
            "Kernel log",
            "Kernel messages at warning or worse in the last hour, repeats merged.",
        ))])
    }

    fn visible_in_bar(&self) -> bool {
        !self.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        iter::once,
        time::{Duration, SystemTime},
    };

    use vetrata_sources::kernel_errors::KernelEntry;

    use super::*;
    use crate::widgets::tooltip_body::SectionBody;

    #[test]
    fn tooltip_lists_latest_messages_oldest_first_after_a_hidden_notice() {
        let errors = KernelOccurrence::merged_oldest_first(
            (0..17)
                .rev()
                .map(|secs| KernelEntry {
                    at: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
                    message: format!("message {secs}"),
                    severity: Severity::Critical,
                })
                .collect::<Vec<_>>(),
        );
        let TooltipBody::Sections(sections) = errors.tooltip() else {
            panic!("expected sections");
        };
        let SectionBody::ColumnTable { records, .. } = sections[0].body() else {
            panic!("expected column table");
        };
        let values: Vec<Cell> = records
            .iter()
            .filter_map(|record| record.cell(1).cloned())
            .collect();
        let expected: Vec<Cell> = once("2 earlier not shown".to_string())
            .chain((2..17).map(|secs| format!("message {secs}")))
            .map(Cell::from)
            .collect();
        assert_eq!(values, expected);
    }
}
