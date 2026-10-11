use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::kernel_errors::{KernelErrors, KernelOccurrence},
    },
    widget_groups::{
        journal_events_group::{JournalEventsMessage, JournalTarget, MESSAGE_CAP},
        terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const TOOLTIP_ROWS: usize = 15;

impl KernelErrors {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, JournalEventsMessage> {
        GroupItem::reading(self, JournalTarget::Kernel).on_press(
            TerminalJournal::PRESS_HINT,
            JournalEventsMessage::OpenLog(TerminalJournal::following(
                config,
                Self::follow_log_args(),
            )),
        )
    }
}

impl KernelOccurrence {
    fn key_value(&self) -> KeyValue {
        let message: String = self.message.chars().take(MESSAGE_CAP).collect();
        let text = match self.count {
            1 => message,
            count => format!("{message} ×{count}"),
        };
        let at = Cell::Time {
            at: self.latest.into(),
            part: TimePart::TimeOfDay,
        };
        KeyValue::new(at, text).level(self.severity)
    }
}

impl BarWidget for KernelErrors {
    fn name(&self) -> &'static str {
        "Kernel log"
    }

    fn icon(&self) -> Glyph {
        Phosphor::LinuxLogo(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        let distinct = self
            .oldest_first
            .iter()
            .filter(|occurrence| occurrence.severity >= Severity::Warning)
            .count();
        u32::try_from(distinct).ok().filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        self.oldest_first
            .iter()
            .map(|occurrence| occurrence.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let hidden = self.oldest_first.len().saturating_sub(TOOLTIP_ROWS);
        let hidden_notice =
            (hidden > 0).then(|| KeyValue::new("", format!("{hidden} earlier not shown")));
        TooltipBody::sections(&[Section::dictionary(
            (
                "Kernel log",
                "Kernel messages at warning or worse in the last hour, repeats merged.",
            ),
            hidden_notice.into_iter().chain(
                self.oldest_first[hidden..]
                    .iter()
                    .map(KernelOccurrence::key_value),
            ),
        )])
    }
}

#[cfg(test)]
mod tests {
    use std::{
        iter::once,
        time::{Duration, SystemTime},
    };

    use super::*;
    use crate::{data::source::kernel_errors::KernelEntry, widgets::tooltip_body::SectionBody};

    #[test]
    fn tooltip_lists_latest_messages_oldest_first_after_a_hidden_notice() {
        let errors = KernelErrors::from_entries(
            (0..17)
                .rev()
                .map(|secs| KernelEntry {
                    at: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
                    message: format!("message {secs}"),
                    severity: Severity::Critical,
                })
                .collect(),
        )
        .expect("errors present");
        let TooltipBody::Sections(sections) = errors.tooltip() else {
            panic!("expected sections");
        };
        let SectionBody::Dictionary(entries) = sections[0].body() else {
            panic!("expected dictionary");
        };
        let values: Vec<Cell> = entries.iter().map(|entry| entry.value().clone()).collect();
        let expected: Vec<Cell> = once("2 earlier not shown".to_string())
            .chain((2..17).map(|secs| format!("message {secs}")))
            .map(Cell::from)
            .collect();
        assert_eq!(values, expected);
    }
}
