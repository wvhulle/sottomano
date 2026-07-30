//! Recent kernel errors as a bar indicator, ported from odin's
//! `kernel-errors.nu`.
//!
//! Read from the systemd journal's kernel transport via libsystemd's blocking
//! reader (the `systemd` crate), so it fits the sync [`Source`] poll — the same
//! off-thread shape as [`systemd_failed`](super::systemd_failed), which reads
//! the systemd Manager over blocking D-Bus. The count is a badge; the tooltip
//! is a timestamped [`Log`](crate::data::source::TooltipBody::Log) of the
//! newest errors. Only errors within [`HORIZON`] are counted, so the widget
//! clears once they age out; an empty window takes it off the bar.

use std::{
    cmp::Reverse,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use systemd::journal::{JournalRecord, OpenOptions};

use crate::{
    data::source::{BarWidget, Level, LogEntry, Source, TooltipBody},
    widgets::icon::Icon,
};

/// Only errors within this window are counted (odin's `horizon`); older ones
/// age out and the widget clears. Matches
/// [`systemd_failed`](super::systemd_failed).
const HORIZON: Duration = Duration::from_secs(3600);
const POLL: Duration = Duration::from_secs(30);
/// Syslog priority for `err`; odin's `-p err` spans `emerg`(0)..=`err`(3).
const ERR_PRIORITY: u8 = 3;
/// Longest message kept per tooltip row (odin's `str substring 0..200`).
const MESSAGE_CAP: usize = 200;
/// Newest rows shown in the tooltip (odin's `first 5`).
const TOOLTIP_ROWS: usize = 5;

#[derive(Debug, Clone)]
struct KernelError {
    /// Wall-clock instant the entry was logged (journal realtime).
    at: SystemTime,
    message: String,
    level: Level,
}

/// Recent kernel errors (syslog priority `err` and above). Only constructed
/// when at least one is within [`HORIZON`].
#[derive(Debug, Clone)]
pub struct KernelErrors {
    errors: Vec<KernelError>,
}

impl BarWidget for KernelErrors {
    fn icon(&self) -> Icon {
        // odin's `icon: "linux"` — the Tux brand glyph.
        Icon::Linux
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.errors.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        // Icon severity is the most severe row, so a crit/emerg line turns the
        // bar red (odin's rule).
        self.errors
            .iter()
            .map(|error| error.level)
            .max()
            .unwrap_or_default()
    }

    fn tooltip(&self) -> TooltipBody {
        let mut errors: Vec<&KernelError> = self.errors.iter().collect();
        errors.sort_by_key(|error| Reverse(error.at));
        TooltipBody::Log(
            errors
                .into_iter()
                .take(TOOLTIP_ROWS)
                .map(|error| LogEntry {
                    at: error.at,
                    message: error.message.clone(),
                    level: error.level,
                })
                .collect(),
        )
    }
}

impl Source for KernelErrors {
    const NAME: &'static str = "kernel-errors";
    const INTERVAL: Duration = POLL;

    fn poll() -> Option<Self> {
        let errors = poll_errors()?;
        (!errors.is_empty()).then_some(Self { errors })
    }
}

/// Kernel-transport journal entries at priority `err`(3) or worse within
/// [`HORIZON`], newest read last. `None` if the journal can't be opened (the
/// widget then stays off the bar); a mid-read error just ends the scan with
/// whatever was collected.
fn poll_errors() -> Option<Vec<KernelError>> {
    let cutoff = SystemTime::now().checked_sub(HORIZON)?;
    let cutoff_usec = u64::try_from(cutoff.duration_since(UNIX_EPOCH).ok()?.as_micros()).ok()?;

    let mut journal = OpenOptions::default().system(true).open().ok()?;
    // odin's `journalctl -k -p err`: the kernel ring buffer, priority err and
    // above. Same-field matches OR together, different fields AND — so this is
    // `_TRANSPORT=kernel AND (PRIORITY=0..=3)`.
    journal.match_add("_TRANSPORT", "kernel").ok()?;
    for priority in 0..=ERR_PRIORITY {
        journal.match_add("PRIORITY", priority.to_string()).ok()?;
    }
    // odin's `--since`: start at the horizon, so aged-out errors never enter.
    journal.seek_realtime_usec(cutoff_usec).ok()?;

    let mut errors = Vec::new();
    while let Ok(Some(record)) = journal.next_entry() {
        let (Some(level), Some(message)) = (row_level(&record), record.get("MESSAGE")) else {
            continue;
        };
        errors.push(KernelError {
            at: journal.timestamp().unwrap_or_else(|_| SystemTime::now()),
            message: message.chars().take(MESSAGE_CAP).collect(),
            level,
        });
    }
    Some(errors)
}

/// A row's severity from its syslog `PRIORITY`: `crit`(2) and worse glow red,
/// plain `err`(3) is amber (odin's `priority <= 2` rule). `None` for anything
/// below `err` or an unparseable priority — the match already excludes those,
/// so this only guards the mapping.
fn row_level(record: &JournalRecord) -> Option<Level> {
    match record.get("PRIORITY")?.parse().ok()? {
        0..=2 => Some(Level::Critical),
        ERR_PRIORITY => Some(Level::Warning),
        _ => None,
    }
}
