use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::oom_kills::{OomKillEntry, OomKiller, OomKills},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::{
        systemd_manager_group::SystemdManagerMessage, terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::{GroupItem, Press},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, Section, TableRow, TooltipBody},
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

impl TableRow for OomKillEntry {
    const HEADINGS: &'static [&'static str] = &["Unit", "Killed by"];

    fn key(&self) -> Cell {
        Cell::Time {
            at: self.at.into(),
            part: TimePart::TimeOfDay,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        vec![self.unit.name().into(), self.killer.label().into()]
    }

    fn level(&self) -> Severity {
        self.severity
    }
}

impl OomKills {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem {
            on_press: Some(Press {
                does: TerminalJournal::PRESS_HINT,
                message: SystemdManagerMessage::OpenLog(TerminalJournal::following(
                    config,
                    OomKills::follow_log_args(),
                )),
            }),
            ..GroupItem::reading(self, HoverTarget::SystemdOomKills)
        }
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
        TooltipBody::sections(&[Section::table(
            (
                "OOM kills",
                "Units whose processes were killed for lack of memory in the last hour.",
            ),
            &self.entries,
        )])
    }
}
