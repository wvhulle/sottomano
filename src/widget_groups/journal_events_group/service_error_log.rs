use crate::{
    config::Config,
    data::source::service_errors::{ServiceErrors, UnitErrors},
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::{
        journal_events_group::{JournalEventsMessage, MESSAGE_CAP},
        terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl ServiceErrors {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, JournalEventsMessage> {
        GroupItem::reading(self, HoverTarget::JournalServiceErrors).on_press(
            TerminalJournal::PRESS_HINT,
            JournalEventsMessage::OpenLog(TerminalJournal::following(
                config,
                self.follow_log_args(),
            )),
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
                    part: TimePart::TimeOfDay,
                }),
                Some(self.count.to_string().into()),
                Some(message.into()),
            ],
        )
        .level(self.severity)
    }
}

impl BarWidget for ServiceErrors {
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
        TooltipBody::sections(&[Section::column_table(
            (
                "Service errors",
                "Journal entries at err or worse from .service units in the last hour, newest \
                 message per service.",
            ),
            ["Latest", "Count", "Message"],
            self.by_unit_oldest_first
                .iter()
                .map(UnitErrors::table_record),
        )])
    }
}
