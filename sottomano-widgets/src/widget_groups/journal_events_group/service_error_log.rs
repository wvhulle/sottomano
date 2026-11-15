use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::journal_service_errors::{ServiceErrors, UnitErrors},
};
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widget_groups::{
        journal_events_group::{
            JournalEventsInteraction, JournalScopeGroup, MESSAGE_CAP, shown_while_alerting,
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

impl JournalScopeGroup {
    pub(super) fn service_errors_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let Some(service_errors) = self
            .service_errors
            .visible_reading(service_errors_panel_visibility)
        else {
            return self.service_errors.failure().map(|failure| {
                GroupItem::reading(failure.reading_row(IconGlyph::default())).with_hover_card(
                    Box::new(move |_| {
                        self.service_errors
                            .sections_described_by(service_errors_hover_card)
                    }),
                )
            });
        };
        Some(
            GroupItem::reading(service_errors_reading_row(service_errors))
                .on_press(
                    TerminalCommand::JOURNAL_PRESS_HINT,
                    JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                        config,
                        service_errors.follow_log_args(),
                    )),
                )
                .with_hover_card(Box::new(move |_| {
                    self.service_errors
                        .sections_described_by(service_errors_hover_card)
                })),
        )
    }
}

impl IntoTableRecord<3> for &UnitErrors {
    fn into_table_record(self) -> TableRecord<3> {
        let message: String = self.latest_message.chars().take(MESSAGE_CAP).collect();
        TableRecord::with_cells(
            self.unit.name(),
            [
                ValueCell::Time {
                    at: self.latest.into(),
                    format: LocalizedTimeFormat::TimeOfDay,
                },
                ValueCell::Hint(self.count.to_string()),
                message.into(),
            ],
        )
        .level(self.severity)
    }
}

pub(super) fn service_errors_state(errors: &ServiceErrors) -> ReadingDisplayState {
    errors
        .by_unit_oldest_first
        .iter()
        .map(|unit_errors| unit_errors.severity)
        .max()
        .unwrap_or_default()
        .into()
}

pub(super) fn service_errors_panel_visibility(errors: &ServiceErrors) -> PanelVisibility {
    shown_while_alerting(
        errors
            .by_unit_oldest_first
            .iter()
            .map(|unit_errors| unit_errors.severity),
    )
}

pub(super) fn service_errors_hover_card(errors: &ServiceErrors) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Service errors",
            "Journal entries at err or worse from .service units in the last hour, newest message \
             per service.",
        ),
        headed_table(
            [
                ColumnHeading::titled("Latest"),
                ColumnHeading::titled("Count"),
                ColumnHeading::titled("Message").align_x(Horizontal::Left),
            ],
            errors.by_unit_oldest_first.iter(),
        ),
    )]
}

pub(super) fn service_errors_reading_row(errors: &ServiceErrors) -> ReadingRow<'_> {
    let base = ReadingRow::glyph_only(
        ServiceErrors::NAME,
        Phosphor::WarningCircle(Weight::Regular).into(),
        service_errors_state(errors),
        Box::new(move |_| service_errors_hover_card(errors)),
    );
    let badge = match errors.by_unit_oldest_first.len() {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    ReadingRow {
        readout: RowReadout {
            badge,
            ..base.readout
        },
        panel_visibility: service_errors_panel_visibility(errors),
        ..base
    }
}
