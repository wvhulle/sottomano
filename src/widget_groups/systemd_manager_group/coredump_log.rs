use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::coredumps::{CoredumpEntry, Coredumps},
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

impl TableRow for CoredumpEntry {
    const HEADINGS: &'static [&'static str] = &["Process", "Signal", "PID", "Unit"];

    fn key(&self) -> Cell {
        Cell::Time {
            at: self.at.into(),
            part: TimePart::TimeOfDay,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            self.command.clone().into(),
            self.signal.clone().into(),
            self.pid.clone().into(),
            self.unit
                .as_ref()
                .map_or(Cell::Missing, |unit| unit.name().into()),
        ]
    }

    fn level(&self) -> Severity {
        self.severity
    }
}

impl Coredumps {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem {
            on_press: Some(Press {
                does: TerminalJournal::PRESS_HINT,
                message: SystemdManagerMessage::OpenLog(TerminalJournal::following(
                    config,
                    Coredumps::follow_log_args(),
                )),
            }),
            ..GroupItem::reading(self, HoverTarget::SystemdCoredumps)
        }
    }
}

impl BarWidget for Coredumps {
    fn icon(&self) -> Glyph {
        Phosphor::Bug(self.state().weight()).into()
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
                "Coredumps",
                "Programs that crashed in the last hour, as recorded by systemd-coredump.",
            ),
            &self.entries,
        )])
    }
}
