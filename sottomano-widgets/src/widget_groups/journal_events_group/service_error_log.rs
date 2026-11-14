use std::borrow::Cow;

use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::journal_service_errors::{ServiceErrors, UnitErrors},
};
use sottomano_toml_config::Config;

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

impl JournalScopeGroup {
    pub(super) fn service_errors_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let target = JournalTarget::ServiceErrors(self.unit_scope);
        let Some(service_errors) = self.service_errors.visible_reading() else {
            return self
                .service_errors
                .failure()
                .map(|failure| GroupItem::reading(failure, target));
        };
        Some(GroupItem::reading(service_errors, target).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                config,
                service_errors.follow_log_args(),
            )),
        ))
    }
}

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
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::WarningCircle(Weight::Regular).into()
    }

    fn badge(&self) -> Option<usize> {
        Some(self.by_unit_oldest_first.len())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.by_unit_oldest_first
            .iter()
            .map(|errors| errors.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::column_table(
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

    fn panel_visibility(&self) -> PanelVisibility {
        shown_while_alerting(
            self.by_unit_oldest_first
                .iter()
                .map(|errors| errors.severity),
        )
    }
}
