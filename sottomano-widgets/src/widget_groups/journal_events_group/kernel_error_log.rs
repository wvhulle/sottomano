use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_kernel_errors::KernelOccurrence;
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::{
        journal_events_group::{
            JournalEventsGroup, JournalEventsInteraction, JournalTarget, MESSAGE_CAP,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

const HOVER_CARD_ROWS: usize = 15;

impl JournalEventsGroup {
    pub(super) fn kernel_log_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let kernel = self.kernel.display()?;
        Some(GroupItem::reading(kernel, JournalTarget::Kernel).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                config,
                KernelOccurrence::follow_log_args(),
            )),
        ))
    }
}

impl IntoTableRecord<2> for &KernelOccurrence {
    fn into_table_record(self) -> TableRecord<2> {
        let message: String = self.message.chars().take(MESSAGE_CAP).collect();
        let at = ValueCell::Time {
            at: self.latest.into(),
            format: LocalizedTimeFormat::TimeOfDay,
        };
        TableRecord::new(
            at,
            [ValueCell::Hint(self.count.to_string()), message.into()],
        )
        .level(self.severity)
    }
}

impl GroupItemReading for Vec<KernelOccurrence> {
    fn name(&self) -> &'static str {
        "Kernel log"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::LinuxLogo(Weight::Regular).into()
    }

    fn badge(&self) -> Option<usize> {
        let distinct = self
            .iter()
            .filter(|occurrence| occurrence.severity >= Severity::Warning)
            .count();
        Some(distinct).filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.iter()
            .map(|occurrence| occurrence.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let hidden_notice = self
            .len()
            .checked_sub(HOVER_CARD_ROWS)
            .filter(|hidden| *hidden > 0)
            .map(|hidden| {
                TableRecord::new(
                    "",
                    [
                        ValueCell::Blank,
                        format!("{hidden} earlier not shown").into(),
                    ],
                )
            });
        HoverCardBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Count"),
                ColumnHeading::titled("Message").align_x(Horizontal::Left),
            ],
            hidden_notice.into_iter().chain(
                self.iter()
                    .rev()
                    .take(HOVER_CARD_ROWS)
                    .rev()
                    .map(IntoTableRecord::into_table_record),
            ),
        )
        .titled((
            "Kernel log",
            "Kernel messages at warning or worse in the last hour, repeats merged.",
        ))])
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.is_empty() {
            PanelVisibility::Hidden
        } else {
            PanelVisibility::Shown
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        iter::once,
        time::{Duration, SystemTime},
    };

    use sottomano_desktop_sources::systemd_units_journal::journal_kernel_errors::KernelEntry;

    use super::*;
    use crate::hover_cards::hover_card_body::SectionBody;

    #[test]
    fn hover_card_lists_latest_messages_oldest_first_after_a_hidden_notice() {
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
        let body = errors.hover_card(Clock::real());
        let sections = body.listed_sections();
        let SectionBody::ColumnTable(table) = sections[0].body() else {
            panic!("expected column table");
        };
        let values: Vec<ValueCell> = table
            .records()
            .filter_map(|record| record.cell(1).cloned())
            .collect();
        let expected: Vec<ValueCell> = once("2 earlier not shown".to_string())
            .chain((2..17).map(|secs| format!("message {secs}")))
            .map(ValueCell::from)
            .collect();
        assert_eq!(values, expected);
    }
}
