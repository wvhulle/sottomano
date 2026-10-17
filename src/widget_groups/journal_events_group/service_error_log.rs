use iced::alignment::Horizontal;

use crate::{
    config::Config,
    data::source::service_errors::{ServiceErrors, UnitErrors},
    widget_groups::{
        journal_events_group::{JournalEventsMessage, JournalTarget, MESSAGE_CAP},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::LocalizedTimeFormat,
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl ServiceErrors {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, JournalEventsMessage> {
        GroupItem::reading(self, JournalTarget::ServiceErrors).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            JournalEventsMessage::OpenLog(TerminalCommand::journal(config, self.follow_log_args())),
        )
    }
}

impl UnitErrors {
    fn table_record(&self) -> TableRecord<3> {
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
        Phosphor::WarningCircle(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.by_unit_oldest_first.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.by_unit_oldest_first
            .iter()
            .map(|errors| errors.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Latest"),
                ColumnHeading::new("Count"),
                ColumnHeading::new("Message").align_x(Horizontal::Left),
            ],
            self.by_unit_oldest_first
                .iter()
                .map(UnitErrors::table_record),
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
