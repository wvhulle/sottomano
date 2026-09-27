use std::{collections::HashMap, time::SystemTime};

use crate::data::source::journal::{KernelEntry, KernelSeverity};

#[derive(Debug, Clone)]
pub struct KernelErrors {
    pub entries: Vec<KernelEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelOccurrence {
    pub latest: SystemTime,
    pub message: String,
    pub count: usize,
    pub severity: KernelSeverity,
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

    pub fn occurrences(&self) -> Vec<KernelOccurrence> {
        let mut by_message: HashMap<&str, KernelOccurrence> = HashMap::new();
        for entry in &self.entries {
            by_message
                .entry(&entry.message)
                .and_modify(|occurrence| {
                    occurrence.count += 1;
                    occurrence.latest = occurrence.latest.max(entry.at);
                    occurrence.severity = occurrence.severity.min(entry.severity);
                })
                .or_insert_with(|| KernelOccurrence {
                    latest: entry.at,
                    message: entry.message.clone(),
                    count: 1,
                    severity: entry.severity,
                });
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

    fn entry(secs: u64, message: &str, severity: KernelSeverity) -> KernelEntry {
        KernelEntry {
            at: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
            message: message.to_string(),
            severity,
        }
    }

    #[test]
    fn repeated_messages_merge_into_one_occurrence_at_their_latest_time() {
        let errors = KernelErrors {
            entries: vec![
                entry(10, "buffer", KernelSeverity::Error),
                entry(30, "buffer", KernelSeverity::Error),
                entry(20, "resume", KernelSeverity::Error),
                entry(40, "noise", KernelSeverity::Warning),
            ],
        };
        assert_eq!(
            errors.occurrences(),
            vec![
                KernelOccurrence {
                    latest: SystemTime::UNIX_EPOCH + Duration::from_secs(20),
                    message: "resume".to_string(),
                    count: 1,
                    severity: KernelSeverity::Error,
                },
                KernelOccurrence {
                    latest: SystemTime::UNIX_EPOCH + Duration::from_secs(30),
                    message: "buffer".to_string(),
                    count: 2,
                    severity: KernelSeverity::Error,
                },
                KernelOccurrence {
                    latest: SystemTime::UNIX_EPOCH + Duration::from_secs(40),
                    message: "noise".to_string(),
                    count: 1,
                    severity: KernelSeverity::Warning,
                },
            ]
        );
    }
}
