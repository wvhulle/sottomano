use std::{convert::Infallible, io, path::PathBuf, time::SystemTime};

use iced::futures::{Stream, StreamExt};
use systemd::journal::{Journal, JournalRecord, OpenOptions};

use crate::data::{
    severity::Severity,
    source::{
        Delivery, LiveSource, Source, delivered,
        journal::{JournalEvent, JournalPriority, OwningUnit, field, journal_windows},
    },
};

impl Source for Vec<CoredumpEntry> {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = CoredumpEntry::NAME;
}

impl LiveSource for Vec<CoredumpEntry> {
    fn events(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        delivered(journal_windows::<CoredumpEntry>().map(Ok))
    }
}

const COREDUMP_MESSAGE_ID: &str = "fc2e22bc6ee647b6b90729ab34a250b1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoredumpEntry {
    pub at: SystemTime,
    pub command: String,
    pub executable: String,
    pub signal: String,
    pub pid: String,
    pub unit: Option<OwningUnit>,
    pub stored_file: Option<PathBuf>,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoredumpInspection {
    Info,
    Debug,
}

impl CoredumpEntry {
    pub fn coredumpctl_args(&self, inspection: CoredumpInspection) -> Vec<String> {
        let verb = match inspection {
            CoredumpInspection::Info => "info",
            CoredumpInspection::Debug => "debug",
        };
        vec!["coredumpctl".to_owned(), verb.to_owned(), self.pid.clone()]
    }

    pub fn latest_of_unit(per_user: bool, unit_name: &str) -> Option<Self> {
        let unit_field = if per_user {
            "COREDUMP_USER_UNIT"
        } else {
            "COREDUMP_UNIT"
        };
        let mut journal = OpenOptions::default()
            .system(true)
            .current_user(per_user)
            .open()
            .inspect_err(|e| tracing::warn!("coredump of {unit_name}: journal open failed: {e}"))
            .ok()?;
        journal.match_add("MESSAGE_ID", COREDUMP_MESSAGE_ID).ok()?;
        journal.match_add(unit_field, unit_name).ok()?;
        journal.seek_tail().ok()?;
        let record = journal.previous_entry().ok()??;
        let at = journal.timestamp().ok()?;
        Self::from_record(&record, at)
    }
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
            executable: field(record, "COREDUMP_EXE")?.to_owned(),
            signal: field(record, "COREDUMP_SIGNAL_NAME")?.to_owned(),
            pid: field(record, "COREDUMP_PID")?.to_owned(),
            unit: OwningUnit::from_fields(record, "COREDUMP_UNIT", "COREDUMP_USER_UNIT"),
            stored_file: field(record, "COREDUMP_FILENAME").map(PathBuf::from),
            severity: JournalPriority::of_record(record)?.severity(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

impl CoredumpEntry {
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
            ("COREDUMP_EXE", "/usr/bin/firefox"),
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
                executable: "/usr/bin/firefox".to_owned(),
                signal: "SIGSEGV".to_owned(),
                pid: "4121".to_owned(),
                unit: Some(OwningUnit::User("app-firefox.scope".to_owned())),
                stored_file: None,
                severity: Severity::Critical,
            })
        );
    }
}
