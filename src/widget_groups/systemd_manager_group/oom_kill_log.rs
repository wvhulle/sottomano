use crate::{
    config::Config,
    data::source::oom_kills::{OomKillEntry, OomKiller, OomKills},
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::{
        systemd_manager_group::SystemdManagerMessage, terminal_journal::TerminalJournal,
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

impl OomKills {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem::reading(self, HoverTarget::SystemdOomKills).on_press(
            TerminalJournal::PRESS_HINT,
            SystemdManagerMessage::OpenLog(TerminalJournal::following(
                config,
                OomKills::follow_log_args(),
            )),
        )
    }
}

impl BarWidget for OomKills {
    fn icon(&self) -> Glyph {
        Phosphor::Memory(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.entries.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.entries
            .iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::column_table(
            (
                "OOM kills",
                "Units whose processes were killed for lack of memory in the last hour.",
            ),
            ["Unit", "Killed by"],
            self.entries.iter().map(OomKillEntry::table_record),
        )])
    }
}
