use iced::alignment::Horizontal;
use sottomano_sources::systemd_units_journal::journal_service_errors::{ServiceErrors, UnitErrors};

use crate::{
    clock::Clock,
    widget_groups::journal_events_group::MESSAGE_CAP,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

impl TableRow<3> for &UnitErrors {
    fn table_record(self) -> TableRecord<3> {
        let message: String = self.latest_message.chars().take(MESSAGE_CAP).collect();
        TableRecord::new(
            self.unit.name(),
            [
                Some(Cell::Time {
                    at: self.latest.into(),
                    format: LocalizedTimeFormat::TimeOfDay,
                }),
                Some(Cell::Hint(self.count.to_string())),
                Some(message.into()),
            ],
        )
        .level(self.severity)
    }
}

impl BarWidget for ServiceErrors {
    fn name(&self) -> &'static str {
        "Service errors"
    }

    fn icon(&self) -> Glyph {
        Phosphor::WarningCircle(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.by_unit_oldest_first.len()).ok()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
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
                ColumnHeading::new("Latest"),
                ColumnHeading::new("Count"),
                ColumnHeading::new("Message").align_x(Horizontal::Left),
            ],
            self.by_unit_oldest_first.iter(),
        )
        .titled((
            "Service errors",
            "Journal entries at err or worse from .service units in the last hour, newest message \
             per service.",
        ))])
    }

    fn visible_in_bar(&self) -> bool {
        !self.is_empty()
    }
}
