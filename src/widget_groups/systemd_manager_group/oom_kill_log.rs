use crate::{
    config::Config,
    data::source::oom_kills::{OomKillEntry, OomKiller},
    widget_groups::{
        systemd_manager_group::{SystemdManagerGroup, SystemdManagerMessage, SystemdTarget},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        group_item::GroupItem,
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl OomKiller {
    fn label(self) -> &'static str {
        match self {
            OomKiller::Kernel => "kernel",
            OomKiller::Oomd => "systemd-oomd",
        }
    }
}

impl OomKillEntry {
    fn table_record(&self) -> TableRecord<2> {
        TableRecord::new(
            Cell::Time {
                at: self.at.into(),
                part: TimePart::TimeOfDay,
            },
            [
                Some(self.unit.name().into()),
                Some(self.killer.label().into()),
            ],
        )
        .level(self.severity)
    }
}

impl SystemdManagerGroup {
    pub(super) fn oom_kill_log_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, SystemdManagerMessage>> {
        let kills = self.oom_kills.present_reading()?;
        Some(GroupItem::reading(kills, SystemdTarget::OomKills).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            SystemdManagerMessage::OpenLog(TerminalCommand::journal(
                config,
                OomKillEntry::follow_log_args(),
            )),
        ))
    }
}

impl Occasional for Vec<OomKillEntry> {
    fn present(&self) -> bool {
        !self.is_empty()
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
            ["Unit", "Killed by"],
            self.iter().map(OomKillEntry::table_record),
        )
        .titled((
            "OOM kills",
            "Units whose processes were killed for lack of memory in the last hour.",
        ))])
    }
}
