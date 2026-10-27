use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    path::PathBuf,
    time::SystemTime,
};

use futures::{Stream, StreamExt};
use libc::{c_int, pid_t};
use serde::Deserialize;
use sottomano_config::severity::Severity;
use systemd::journal::{Journal, OpenOptions};

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::journal_follow::{
        JournalEvent, JournalFollowError, decode_record, journal_windows,
    },
    systemd_units_journal::journal_record_fields::{
        CoredumpUnit, JournalPriority, MessageId, OwningUnit,
    },
};

impl SnapshotSource for Vec<CoredumpEntry> {
    type Error = JournalFollowError;
    type StreamInputs = ();

    const NAME: &'static str = "Coredumps";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        journal_windows::<CoredumpEntry>().map(SnapshotDelivery::live_push)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "c_int")]
pub enum CoreDumpingSignal {
    Quit,
    Illegal,
    Trap,
    Abort,
    Bus,
    FloatingPoint,
    Segfault,
    BadSyscall,
    CpuLimit,
    FileSizeLimit,
}

impl CoreDumpingSignal {
    fn name(self) -> &'static str {
        match self {
            CoreDumpingSignal::Quit => "SIGQUIT",
            CoreDumpingSignal::Illegal => "SIGILL",
            CoreDumpingSignal::Trap => "SIGTRAP",
            CoreDumpingSignal::Abort => "SIGABRT",
            CoreDumpingSignal::Bus => "SIGBUS",
            CoreDumpingSignal::FloatingPoint => "SIGFPE",
            CoreDumpingSignal::Segfault => "SIGSEGV",
            CoreDumpingSignal::BadSyscall => "SIGSYS",
            CoreDumpingSignal::CpuLimit => "SIGXCPU",
            CoreDumpingSignal::FileSizeLimit => "SIGXFSZ",
        }
    }
}

impl Display for CoreDumpingSignal {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotACoreDumpingSignal(c_int);

impl Display for NotACoreDumpingSignal {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "signal {} does not dump core", self.0)
    }
}

impl TryFrom<c_int> for CoreDumpingSignal {
    type Error = NotACoreDumpingSignal;

    fn try_from(number: c_int) -> Result<Self, Self::Error> {
        match number {
            libc::SIGQUIT => Ok(CoreDumpingSignal::Quit),
            libc::SIGILL => Ok(CoreDumpingSignal::Illegal),
            libc::SIGTRAP => Ok(CoreDumpingSignal::Trap),
            libc::SIGABRT => Ok(CoreDumpingSignal::Abort),
            libc::SIGBUS => Ok(CoreDumpingSignal::Bus),
            libc::SIGFPE => Ok(CoreDumpingSignal::FloatingPoint),
            libc::SIGSEGV => Ok(CoreDumpingSignal::Segfault),
            libc::SIGSYS => Ok(CoreDumpingSignal::BadSyscall),
            libc::SIGXCPU => Ok(CoreDumpingSignal::CpuLimit),
            libc::SIGXFSZ => Ok(CoreDumpingSignal::FileSizeLimit),
            _ => Err(NotACoreDumpingSignal(number)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CoredumpRecord {
    #[serde(rename = "COREDUMP_COMM")]
    command: String,
    #[serde(rename = "COREDUMP_EXE")]
    executable: PathBuf,
    #[serde(rename = "COREDUMP_SIGNAL")]
    signal: CoreDumpingSignal,
    #[serde(rename = "COREDUMP_PID")]
    pid: pid_t,
    #[serde(flatten)]
    unit: CoredumpUnit,
    #[serde(rename = "COREDUMP_FILENAME")]
    stored_file: Option<PathBuf>,
    #[serde(rename = "PRIORITY")]
    priority: JournalPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoredumpEntry {
    pub at: SystemTime,
    pub command: String,
    pub executable: PathBuf,
    pub signal: CoreDumpingSignal,
    pub pid: pid_t,
    pub unit: Option<OwningUnit>,
    pub stored_file: Option<PathBuf>,
    pub severity: Severity,
}

impl CoredumpEntry {
    fn stamped(record: CoredumpRecord, at: SystemTime) -> Self {
        CoredumpEntry {
            at,
            command: record.command,
            executable: record.executable,
            signal: record.signal,
            pid: record.pid,
            unit: record.unit.owning_unit(),
            stored_file: record.stored_file,
            severity: record.priority.severity(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoredumpInspection {
    Info,
    Debug,
}

impl CoredumpEntry {
    #[must_use]
    pub fn coredumpctl_args(&self, inspection: CoredumpInspection) -> Vec<String> {
        let verb = match inspection {
            CoredumpInspection::Info => "info",
            CoredumpInspection::Debug => "debug",
        };
        vec![
            "coredumpctl".to_owned(),
            verb.to_owned(),
            self.pid.to_string(),
        ]
    }

    #[must_use]
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
        MessageId::COREDUMP.add_match(&mut journal).ok()?;
        journal.match_add(unit_field, unit_name).ok()?;
        journal.seek_tail().ok()?;
        let record = journal.previous_entry().ok()??;
        let at = journal.timestamp().ok()?;
        let record = decode_record(record)
            .inspect_err(|e| tracing::warn!("coredump of {unit_name}: malformed record: {e}"))
            .ok()?;
        Some(Self::stamped(record, at))
    }
}

impl JournalEvent for CoredumpEntry {
    type Record = CoredumpRecord;

    const NAME: &'static str = "coredump";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        MessageId::COREDUMP.add_match(journal)
    }

    fn admit(record: CoredumpRecord, at: SystemTime) -> Option<Self> {
        Some(Self::stamped(record, at))
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
            .chain([MessageId::COREDUMP.follow_match()])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use systemd::journal::JournalRecord;

    use super::*;
    use crate::system_protocol_clients::journal_follow::journal_record;

    fn firefox_coredump<'a>(
        varying_fields: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> JournalRecord {
        journal_record(
            [
                ("MESSAGE_ID", "fc2e22bc6ee647b6b90729ab34a250b1"),
                ("PRIORITY", "2"),
                ("COREDUMP_COMM", "firefox"),
                ("COREDUMP_EXE", "/usr/bin/firefox"),
                ("COREDUMP_SIGNAL_NAME", "SIGSEGV"),
                ("COREDUMP_UNIT", "user@1000.service"),
                ("COREDUMP_USER_UNIT", "app-firefox.scope"),
            ]
            .into_iter()
            .chain(varying_fields),
        )
    }

    #[test]
    fn a_user_process_coredump_belongs_to_its_user_unit() {
        let record = firefox_coredump([("COREDUMP_SIGNAL", "11"), ("COREDUMP_PID", "4121")]);
        assert_eq!(
            decode_record(record)
                .map(|record| CoredumpEntry::stamped(record, SystemTime::UNIX_EPOCH)),
            Ok(CoredumpEntry {
                at: SystemTime::UNIX_EPOCH,
                command: "firefox".to_owned(),
                executable: PathBuf::from("/usr/bin/firefox"),
                signal: CoreDumpingSignal::Segfault,
                pid: 4121,
                unit: Some(OwningUnit::User("app-firefox.scope".to_owned())),
                stored_file: None,
                severity: Severity::Critical,
            })
        );
    }

    #[test]
    fn an_abort_decodes_from_its_signal_number_and_a_textual_pid_is_malformed() {
        let abort: CoredumpRecord = decode_record(firefox_coredump([
            ("COREDUMP_SIGNAL", "6"),
            ("COREDUMP_PID", "4121"),
        ]))
        .expect("a well-formed coredump");
        assert_eq!(abort.signal, CoreDumpingSignal::Abort);
        assert!(
            decode_record::<CoredumpRecord>(firefox_coredump([
                ("COREDUMP_SIGNAL", "6"),
                ("COREDUMP_PID", "firefox")
            ]))
            .is_err()
        );
        assert!(
            decode_record::<CoredumpRecord>(firefox_coredump([
                ("COREDUMP_SIGNAL", "15"),
                ("COREDUMP_PID", "4121")
            ]))
            .is_err()
        );
    }
}
