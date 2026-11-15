use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_kernel_errors::KernelOccurrence;
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTable, SectionTitle, headed_table},
    widget_groups::{
        journal_events_group::{
            JournalEventsInteraction, JournalScopeGroup, JournalTarget, MESSAGE_CAP,
            shown_while_alerting,
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

impl JournalScopeGroup {
    pub(super) fn kernel_log_item(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let kernel = self.kernel.reading_row(
            IconGlyph::default(),
            |snapshot| kernel_log_reading_row(snapshot),
            clock,
        )?;
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
        TableRecord::with_cells(
            at,
            [ValueCell::Hint(self.count.to_string()), message.into()],
        )
        .level(self.severity)
    }
}

pub(super) fn kernel_log_state(occurrences: &[KernelOccurrence]) -> ReadingDisplayState {
    occurrences
        .iter()
        .map(|occurrence| occurrence.severity)
        .max()
        .unwrap_or_default()
        .into()
}

pub(super) fn kernel_log_panel_visibility(occurrences: &[KernelOccurrence]) -> PanelVisibility {
    shown_while_alerting(occurrences.iter().map(|occurrence| occurrence.severity))
}

pub(super) fn kernel_log_reading_row(occurrences: &[KernelOccurrence]) -> ReadingRow<'_> {
    let warning_count = occurrences
        .iter()
        .filter(|occurrence| occurrence.severity >= Severity::Warning)
        .count();
    let badge = match warning_count {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Ungauged,
            badge,
        },
        panel_visibility: kernel_log_panel_visibility(occurrences),
        ..ReadingRow::glyph_only(
            "Kernel log",
            Phosphor::LinuxLogo(Weight::Regular).into(),
            kernel_log_state(occurrences),
            Box::new(move |_| kernel_log_hover_card(occurrences)),
        )
    }
}

pub(super) fn kernel_log_hover_card(occurrences: &[KernelOccurrence]) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Messages",
            "Kernel messages at warning or worse in the last hour, repeats merged.",
        ),
        kernel_message_table(occurrences),
    )]
}

fn kernel_message_table(occurrences: &[KernelOccurrence]) -> SectionTable {
    let hidden_notice = occurrences
        .len()
        .checked_sub(HOVER_CARD_ROWS)
        .filter(|hidden| *hidden > 0)
        .map(|hidden| {
            TableRecord::with_cells(
                "",
                [
                    ValueCell::Blank,
                    format!("{hidden} earlier not shown").into(),
                ],
            )
        });
    headed_table(
        [
            ColumnHeading::titled("Count"),
            ColumnHeading::titled("Message").align_x(Horizontal::Left),
        ],
        hidden_notice.into_iter().chain(
            occurrences
                .iter()
                .rev()
                .take(HOVER_CARD_ROWS)
                .rev()
                .map(IntoTableRecord::into_table_record),
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        iter::once,
        time::{Duration, SystemTime},
    };

    use sottomano_desktop_sources::systemd_units_journal::journal_kernel_errors::KernelEntry;

    use super::*;

    fn kernel_occurrences_at(severities: &[Severity]) -> Vec<KernelOccurrence> {
        KernelOccurrence::merged_oldest_first(
            severities
                .iter()
                .enumerate()
                .map(|(message_index, severity)| KernelEntry {
                    at: SystemTime::UNIX_EPOCH,
                    message: format!("message {message_index}"),
                    severity: *severity,
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn kernel_log_hidden_until_an_entry_reaches_warning() {
        assert_eq!(
            kernel_log_panel_visibility(&kernel_occurrences_at(&[Severity::Normal, Severity::Low])),
            PanelVisibility::Hidden
        );
        assert_eq!(
            kernel_log_panel_visibility(&kernel_occurrences_at(&[
                Severity::Normal,
                Severity::Warning
            ])),
            PanelVisibility::Shown
        );
    }

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
        let table = kernel_message_table(&errors);
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
