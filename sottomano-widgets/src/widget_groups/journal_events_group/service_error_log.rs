use iced::alignment::Horizontal;
use sottomano_sources::systemd_units_journal::journal_service_errors::{ServiceErrors, UnitErrors};

use crate::{
    clock::Clock,
    widget_groups::journal_events_group::MESSAGE_CAP,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::{Section, TooltipBody},
        value_cells::ValueCell,
    },
};

impl IntoTableRecord<3> for &UnitErrors {
    fn into_table_record(self) -> TableRecord<3> {
        let message: String = self.latest_message.chars().take(MESSAGE_CAP).collect();
        TableRecord::new(
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

impl GroupItemReading for ServiceErrors {
    fn name(&self) -> &'static str {
        "Service errors"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::WarningCircle(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.by_unit_oldest_first.len()).ok()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.by_unit_oldest_first
            .iter()
            .map(|errors| errors.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Latest"),
                ColumnHeading::titled("Count"),
                ColumnHeading::titled("Message").align_x(Horizontal::Left),
            ],
            self.by_unit_oldest_first.iter(),
        )
        .titled((
            "Service errors",
            "Journal entries at err or worse from .service units in the last hour, newest message \
             per service.",
        ))])
    }

    fn visible_on_panel(&self) -> bool {
        !self.is_empty()
    }
}
