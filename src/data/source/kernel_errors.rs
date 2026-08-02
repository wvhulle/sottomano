use std::cmp::Reverse;

use crate::data::source::{
    BarWidget, Glyph, Level, LogEntry, TooltipBody, font_awesome::Brands, journal::KernelEntry,
};

const MESSAGE_CAP: usize = 200;
const TOOLTIP_ROWS: usize = 5;
const CRITICAL_MAX_PRIORITY: u8 = 2;

#[derive(Debug, Clone)]
pub struct KernelErrors {
    entries: Vec<KernelEntry>,
}

impl KernelErrors {
    pub fn from_entries(entries: Vec<KernelEntry>) -> Option<Self> {
        (!entries.is_empty()).then_some(Self { entries })
    }

    pub fn follow_log_args() -> Vec<String> {
        ["-k", "-b", "-f", "-p", "warning"]
            .map(String::from)
            .to_vec()
    }
}

impl BarWidget for KernelErrors {
    fn icon(&self) -> Glyph {
        Brands::Linux.into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.entries.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        self.entries
            .iter()
            .map(|entry| priority_level(entry.priority))
            .max()
            .unwrap_or_default()
    }

    fn tooltip(&self) -> TooltipBody {
        let mut entries: Vec<&KernelEntry> = self.entries.iter().collect();
        entries.sort_by_key(|entry| Reverse(entry.at));
        TooltipBody::Log(
            entries
                .into_iter()
                .take(TOOLTIP_ROWS)
                .map(|entry| LogEntry {
                    at: entry.at,
                    message: entry.message.chars().take(MESSAGE_CAP).collect(),
                    level: priority_level(entry.priority),
                })
                .collect(),
        )
    }
}

fn priority_level(priority: u8) -> Level {
    if priority <= CRITICAL_MAX_PRIORITY {
        Level::Critical
    } else {
        Level::Warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_this_boot_at_warning_and_above() {
        assert_eq!(
            KernelErrors::follow_log_args(),
            ["-k", "-b", "-f", "-p", "warning"]
        );
    }
}
