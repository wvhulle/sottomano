use std::{collections::HashMap, io, time::SystemTime};

use iced::futures::{Stream, StreamExt};
use serde::Deserialize;
use systemd::journal::Journal;

use crate::data::{
    severity::Severity,
    source::{
        DeliveryCadence, ReadingSource, SourceDelivery,
        journal::{
            JournalEvent, JournalFollowError, JournalPriority, JournalTransport, LoggingUnit,
            MessageId, OwningUnit, either_of, journal_windows,
        },
    },
};

impl ReadingSource for ServiceErrors {
    type Error = JournalFollowError;
    type StreamInputs = ();

    const NAME: &'static str = ServiceErrorEntry::NAME;
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(_key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        journal_windows::<ServiceErrorEntry>()
            .map(|entries| entries.map(ServiceErrors::from))
            .map(SourceDelivery::Reading)
    }
}

const SERVICE_UNIT_SUFFIX: &str = ".service";

#[derive(Debug, Deserialize)]
pub struct ServiceErrorRecord {
    #[serde(rename = "MESSAGE")]
    message: String,
    #[serde(rename = "PRIORITY")]
    priority: JournalPriority,
    #[serde(rename = "_TRANSPORT")]
    transport: JournalTransport,
    #[serde(rename = "MESSAGE_ID")]
    message_id: Option<MessageId>,
    #[serde(flatten)]
    unit: LoggingUnit,
}

impl ServiceErrorRecord {
    fn reported_elsewhere(&self) -> bool {
        self.transport == JournalTransport::Kernel || self.message_id == Some(MessageId::COREDUMP)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceErrorEntry {
    pub at: SystemTime,
    pub unit: OwningUnit,
    pub message: String,
    pub severity: Severity,
}

impl JournalEvent for ServiceErrorEntry {
    type Record = ServiceErrorRecord;

    const NAME: &'static str = "service error";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        JournalPriority::Error.match_this_urgent_or_more(journal)
    }

    fn admit(record: ServiceErrorRecord, at: SystemTime) -> Option<Self> {
        if record.reported_elsewhere() {
            return None;
        }
        let unit = record
            .unit
            .owning_unit()
            .filter(|unit| unit.name().ends_with(SERVICE_UNIT_SUFFIX))?;
        Some(ServiceErrorEntry {
            at,
            unit,
            message: record.message,
            severity: record.priority.severity(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitErrors {
    pub unit: OwningUnit,
    pub latest: SystemTime,
    pub latest_message: String,
    pub count: usize,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub struct ServiceErrors {
    pub entry_count: usize,
    pub by_unit_oldest_first: Vec<UnitErrors>,
}

impl From<Vec<ServiceErrorEntry>> for ServiceErrors {
    fn from(entries: Vec<ServiceErrorEntry>) -> Self {
        Self {
            entry_count: entries.len(),
            by_unit_oldest_first: errors_by_unit(entries),
        }
    }
}

impl ServiceErrors {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    pub fn follow_log_args(&self) -> Vec<String> {
        ["-b", "-f", "-n", "all", "-p", "err"]
            .map(String::from)
            .into_iter()
            .chain(either_of(
                self.by_unit_oldest_first
                    .iter()
                    .map(|errors| errors.unit.logged_by_match()),
            ))
            .collect()
    }
}

fn errors_by_unit(entries: Vec<ServiceErrorEntry>) -> Vec<UnitErrors> {
    let mut by_unit: HashMap<OwningUnit, UnitErrors> = HashMap::new();
    for entry in entries {
        if let Some(errors) = by_unit.get_mut(&entry.unit) {
            errors.count += 1;
            errors.severity = errors.severity.max(entry.severity);
            if entry.at >= errors.latest {
                errors.latest = entry.at;
                errors.latest_message = entry.message;
            }
        } else {
            by_unit.insert(
                entry.unit.clone(),
                UnitErrors {
                    unit: entry.unit,
                    latest: entry.at,
                    latest_message: entry.message,
                    count: 1,
                    severity: entry.severity,
                },
            );
        }
    }
    let mut units: Vec<_> = by_unit.into_values().collect();
    units.sort_by_key(|errors| errors.latest);
    units
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::source::journal::{decode_record, journal_record};

    fn admitted<'a>(
        fields: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Option<ServiceErrorEntry> {
        let record = decode_record(journal_record(fields)).expect("a well-formed record");
        ServiceErrorEntry::admit(record, SystemTime::UNIX_EPOCH)
    }

    fn user_unit_error(user_unit: &str) -> Option<ServiceErrorEntry> {
        admitted([
            ("PRIORITY", "3"),
            ("MESSAGE", "dial tcp: connection refused"),
            ("_TRANSPORT", "stdout"),
            ("_SYSTEMD_UNIT", "user@1000.service"),
            ("_SYSTEMD_USER_UNIT", user_unit),
        ])
    }

    #[test]
    fn a_coredump_report_is_left_to_the_coredump_log() {
        assert_eq!(
            admitted([
                ("PRIORITY", "2"),
                (
                    "MESSAGE",
                    "Process 17041 (vetrata) of user 1000 dumped core.\n\nModule …"
                ),
                ("MESSAGE_ID", "fc2e22bc6ee647b6b90729ab34a250b1"),
                ("_TRANSPORT", "journal"),
                (
                    "_SYSTEMD_UNIT",
                    "systemd-coredump@0-25343-38256_38125-0.service"
                ),
            ]),
            None
        );
    }

    #[test]
    fn an_error_logged_by_a_user_service_belongs_to_that_service_but_a_scope_is_no_service() {
        assert_eq!(
            user_unit_error("syncthing.service"),
            Some(ServiceErrorEntry {
                at: SystemTime::UNIX_EPOCH,
                unit: OwningUnit::User("syncthing.service".to_owned()),
                message: "dial tcp: connection refused".to_owned(),
                severity: Severity::Warning,
            })
        );
        assert_eq!(user_unit_error("kitty-1.scope"), None);
    }
}
