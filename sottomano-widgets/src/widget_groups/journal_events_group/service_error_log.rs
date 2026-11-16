use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::journal_service_errors::{ServiceErrors, UnitErrors},
};

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge},
        indicator_state::IndicatorState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widget_groups::journal_events_group::{JournalScopeGroup, MESSAGE_CAP, shown_while_alerting},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

pub(super) const SERVICE_ERRORS_GLYPH: Phosphor = Phosphor::WarningCircle(Weight::Regular);

impl JournalScopeGroup {
    pub(super) fn service_errors_item<'a>(
        &'a self,
        service_errors_row: ReadingRow<'a>,
    ) -> GroupIndicator<'a> {
        GroupIndicator::sample(service_errors_row).with_hover_card(Box::new(move |_| {
            self.service_errors
                .sections_described_by(service_errors_hover_card)
        }))
    }
}

impl From<&UnitErrors> for TableRecord<3> {
    fn from(unit_errors: &UnitErrors) -> Self {
        let message: String = unit_errors
            .latest_message
            .chars()
            .take(MESSAGE_CAP)
            .collect();
        TableRecord::with_cells(
            unit_errors.unit.name(),
            [
                ValueCell::Time {
                    at: unit_errors.latest.into(),
                    format: LocalizedTimeFormat::TimeOfDay,
                },
                ValueCell::Hint(unit_errors.count.to_string()),
                message.into(),
            ],
        )
        .level(unit_errors.severity)
    }
}

pub(super) fn service_errors_state(errors: &ServiceErrors) -> IndicatorState {
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

pub(super) fn service_errors_indicator(errors: &ServiceErrors) -> ReadingRow<'_> {
    let base = ReadingRow::glyph_only(
        ServiceErrors::NAME,
        SERVICE_ERRORS_GLYPH.into(),
        service_errors_state(errors),
        Box::new(move |_| service_errors_hover_card(errors)),
    );
    let badge = match errors.by_unit_oldest_first.len() {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    ReadingRow {
        readout: IndicatorReadout {
            badge,
            ..base.readout
        },
        panel_visibility: service_errors_panel_visibility(errors),
        ..base
    }
}
