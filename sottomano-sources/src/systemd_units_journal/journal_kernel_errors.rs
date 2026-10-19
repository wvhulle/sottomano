use std::{collections::HashMap, io, time::SystemTime};

use futures::{Stream, StreamExt};
use serde::Deserialize;
use sottomano_config::severity::Severity;
use systemd::journal::Journal;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    systemd_units_journal::journal_follow::{
        JournalEvent, JournalFollowError, JournalPriority, JournalTransport, journal_windows,
    },
};

impl ReadingSource for Vec<KernelOccurrence> {
    type Error = JournalFollowError;
    type StreamInputs = ();

    const NAME: &'static str = KernelEntry::NAME;
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(_key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        journal_windows::<KernelEntry>()
            .map(|entries| entries.map(KernelOccurrence::merged_oldest_first))
            .map(SourceDelivery::Reading)
    }
}

#[derive(Debug, Clone)]
pub struct KernelEntry {
    pub at: SystemTime,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Deserialize)]
pub struct KernelRecord {
    #[serde(rename = "MESSAGE")]
    message: String,
    #[serde(rename = "PRIORITY")]
    priority: JournalPriority,
}

impl JournalEvent for KernelEntry {
    type Record = KernelRecord;

    const NAME: &'static str = "kernel";
    const OPENS_USER_JOURNALS: bool = false;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        JournalTransport::Kernel.add_match(journal)?;
        JournalPriority::Warning.match_this_urgent_or_more(journal)
    }

    fn admit(record: KernelRecord, at: SystemTime) -> Option<Self> {
        Some(KernelEntry {
            at,
            message: record.message,
            severity: record.priority.severity(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelOccurrence {
    pub latest: SystemTime,
    pub message: String,
    pub count: usize,
    pub severity: Severity,
}

impl KernelOccurrence {
    pub fn follow_log_args() -> Vec<String> {
        ["-k", "-b", "-f", "-p", "warning"]
            .map(String::from)
            .to_vec()
    }

    #[must_use]
    pub fn merged_oldest_first(entries: Vec<KernelEntry>) -> Vec<Self> {
        let mut by_message: HashMap<String, Self> = HashMap::new();
        for entry in entries {
            if let Some(occurrence) = by_message.get_mut(&entry.message) {
                occurrence.count += 1;
                occurrence.latest = occurrence.latest.max(entry.at);
                occurrence.severity = occurrence.severity.max(entry.severity);
            } else {
                by_message.insert(
                    entry.message.clone(),
                    KernelOccurrence {
                        latest: entry.at,
                        message: entry.message,
                        count: 1,
                        severity: entry.severity,
                    },
                );
            }
        }
        let mut occurrences: Vec<_> = by_message.into_values().collect();
        occurrences.sort_by_key(|occurrence| occurrence.latest);
        occurrences
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn entry(secs: u64, message: &str, severity: Severity) -> KernelEntry {
        KernelEntry {
            at: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
            message: message.to_string(),
            severity,
        }
    }

    #[test]
    fn repeated_messages_merge_into_one_occurrence_at_their_latest_time() {
        let oldest_first = KernelOccurrence::merged_oldest_first(vec![
            entry(10, "buffer", Severity::Warning),
            entry(30, "buffer", Severity::Critical),
            entry(20, "resume", Severity::Warning),
            entry(40, "noise", Severity::Normal),
        ]);
        assert_eq!(
            oldest_first,
            vec![
                KernelOccurrence {
                    latest: SystemTime::UNIX_EPOCH + Duration::from_secs(20),
                    message: "resume".to_string(),
                    count: 1,
                    severity: Severity::Warning,
                },
                KernelOccurrence {
                    latest: SystemTime::UNIX_EPOCH + Duration::from_secs(30),
                    message: "buffer".to_string(),
                    count: 2,
                    severity: Severity::Critical,
                },
                KernelOccurrence {
                    latest: SystemTime::UNIX_EPOCH + Duration::from_secs(40),
                    message: "noise".to_string(),
                    count: 1,
                    severity: Severity::Normal,
                },
            ]
        );
    }
}
