use std::{io, time::SystemTime};

use systemd::journal::{Journal, JournalRecord};

use crate::data::{
    severity::Severity,
    source::journal::{JournalEvent, JournalPriority, OwningUnit, field},
};

const COREDUMP_MESSAGE_ID: &str = "fc2e22bc6ee647b6b90729ab34a250b1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoredumpEntry {
    pub at: SystemTime,
    pub command: String,
    pub signal: String,
    pub pid: String,
    pub unit: Option<OwningUnit>,
    pub severity: Severity,
}

impl JournalEvent for CoredumpEntry {
    const NAME: &'static str = "coredump";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        journal.match_add("MESSAGE_ID", COREDUMP_MESSAGE_ID)?;
        Ok(())
    }

    fn from_record(record: &JournalRecord, at: SystemTime) -> Option<Self> {
        Some(CoredumpEntry {
            at,
            command: field(record, "COREDUMP_COMM")?.to_owned(),
            signal: field(record, "COREDUMP_SIGNAL_NAME")?.to_owned(),
            pid: field(record, "COREDUMP_PID")?.to_owned(),
            unit: OwningUnit::from_fields(record, "COREDUMP_UNIT", "COREDUMP_USER_UNIT"),
            severity: JournalPriority::of_record(record)?.severity(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

#[derive(Debug, Clone)]
pub struct Coredumps {
    pub entries: Vec<CoredumpEntry>,
}

impl Coredumps {
    pub fn from_entries(entries: Vec<CoredumpEntry>) -> Option<Self> {
        (!entries.is_empty()).then_some(Self { entries })
    }

    pub fn follow_log_args() -> Vec<String> {
        ["-b", "-f", "-n", "all"]
            .map(String::from)
            .into_iter()
            .chain([format!("MESSAGE_ID={COREDUMP_MESSAGE_ID}")])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_user_process_coredump_belongs_to_its_user_unit() {
        let record: JournalRecord = [
            ("MESSAGE_ID", COREDUMP_MESSAGE_ID),
            ("PRIORITY", "2"),
            ("COREDUMP_COMM", "firefox"),
            ("COREDUMP_SIGNAL_NAME", "SIGSEGV"),
            ("COREDUMP_PID", "4121"),
            ("COREDUMP_UNIT", "user@1000.service"),
            ("COREDUMP_USER_UNIT", "app-firefox.scope"),
        ]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .into();
        assert_eq!(
            CoredumpEntry::from_record(&record, SystemTime::UNIX_EPOCH),
            Some(CoredumpEntry {
                at: SystemTime::UNIX_EPOCH,
                command: "firefox".to_owned(),
                signal: "SIGSEGV".to_owned(),
                pid: "4121".to_owned(),
                unit: Some(OwningUnit::User("app-firefox.scope".to_owned())),
                severity: Severity::Critical,
            })
        );
    }
}
