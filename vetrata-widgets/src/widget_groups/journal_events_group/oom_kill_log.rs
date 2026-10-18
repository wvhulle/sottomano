use iced::alignment::Horizontal;
use vetrata_config::Config;
use vetrata_sources::oom_kills::{OomKillEntry, OomKiller};

use crate::{
    widget_groups::{
        journal_events_group::{JournalEventsGroup, JournalEventsMessage, JournalTarget},
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

impl TableRow<2> for &OomKillEntry {
    fn table_record(self) -> TableRecord<2> {
        let killer = match self.killer {
            OomKiller::Kernel => "kernel",
            OomKiller::Oomd => "systemd-oomd",
        };
        TableRecord::new(
            Cell::Time {
                at: self.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [Some(self.unit.name().into()), Some(killer.into())],
        )
        .level(self.severity)
    }
}

impl JournalEventsGroup {
    pub(super) fn oom_kill_log_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsMessage>> {
        let kills = self.oom_kills.display()?;
        Some(GroupItem::reading(kills, JournalTarget::OomKills).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            JournalEventsMessage::OpenLog(TerminalCommand::journal(
                config,
                OomKillEntry::follow_log_args(),
            )),
        ))
    }
}

impl BarWidget for Vec<OomKillEntry> {
    fn name(&self) -> &'static str {
        "OOM kills"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Memory(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Unit").align_x(Horizontal::Left),
                ColumnHeading::new("Killed by").align_x(Horizontal::Left),
            ],
            self.iter(),
        )
        .titled((
            "OOM kills",
            "Units whose processes were killed for lack of memory in the last hour.",
        ))])
    }

    fn visible_in_bar(&self) -> bool {
        !self.is_empty()
    }
}
