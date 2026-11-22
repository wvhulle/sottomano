use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    path::PathBuf,
    time::SystemTime,
};

use futures::{Stream, StreamExt};
use rustix::process::Signal;
use serde::Deserialize;
use sottomano_toml_config::severity::Severity;
use systemd::journal::{Journal, OpenOptions};

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::journal_follow::{
        JournalEvent, JournalFollowError, decode_record, journal_windows,
    },
    systemd_units_journal::{
        SystemdUnitScope,
        journal_record_fields::{CoredumpUnit, JournalPriority, MessageId, OwningUnit},
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
#[serde(try_from = "i32")]
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotACoreDumpingSignal {
    signal_number: i32,
}

impl Display for NotACoreDumpingSignal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(
            formatter,
            "signal {} does not dump core",
            self.signal_number
        )
    }
}

impl TryFrom<i32> for CoreDumpingSignal {
    type Error = NotACoreDumpingSignal;

    fn try_from(number: i32) -> Result<Self, Self::Error> {
        match Signal::from_named_raw(number) {
            Some(Signal::QUIT) => Ok(CoreDumpingSignal::Quit),
            Some(Signal::ILL) => Ok(CoreDumpingSignal::Illegal),
            Some(Signal::TRAP) => Ok(CoreDumpingSignal::Trap),
            Some(Signal::ABORT) => Ok(CoreDumpingSignal::Abort),
            Some(Signal::BUS) => Ok(CoreDumpingSignal::Bus),
            Some(Signal::FPE) => Ok(CoreDumpingSignal::FloatingPoint),
            Some(Signal::SEGV) => Ok(CoreDumpingSignal::Segfault),
            Some(Signal::SYS) => Ok(CoreDumpingSignal::BadSyscall),
            Some(Signal::XCPU) => Ok(CoreDumpingSignal::CpuLimit),
            Some(Signal::XFSZ) => Ok(CoreDumpingSignal::FileSizeLimit),
            Some(_) | None => Err(NotACoreDumpingSignal {
                signal_number: number,
            }),
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
    pid: i32,
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
    pub pid: i32,
    pub unit: CoredumpOwner,
    pub core_file: CoreFileStorage,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoredumpOwner {
    Unit(OwningUnit),
    OutsideUnits,
}

impl CoredumpOwner {
    #[must_use]
    pub const fn unit_scope(&self) -> SystemdUnitScope {
        match self {
            CoredumpOwner::Unit(unit) => unit.unit_scope(),
            CoredumpOwner::OutsideUnits => SystemdUnitScope::SystemWide,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreFileStorage {
    OnDisk(PathBuf),
    NotOnDisk,
}

impl CoredumpEntry {
    fn stamped(record: CoredumpRecord, at: SystemTime) -> Self {
        CoredumpEntry {
            at,
            command: record.command,
            executable: record.executable,
            signal: record.signal,
            pid: record.pid,
            unit: record.unit.coredump_owner(),
            core_file: record
                .stored_file
                .map_or(CoreFileStorage::NotOnDisk, CoreFileStorage::OnDisk),
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
    pub fn latest_of_unit(unit_scope: SystemdUnitScope, unit_name: &str) -> Option<Self> {
        let unit_field = match unit_scope {
            SystemdUnitScope::UserSession => "COREDUMP_USER_UNIT",
            SystemdUnitScope::SystemWide => "COREDUMP_UNIT",
        };
        let mut journal = OpenOptions::default()
            .system(true)
            .current_user(unit_scope == SystemdUnitScope::UserSession)
            .open()
            .inspect_err(|error| {
                tracing::warn!("coredump of {unit_name}: journal open failed: {error}");
            })
            .ok()?;
        let mut latest_entry = || -> io::Result<_> {
            MessageId::COREDUMP.add_match(&mut journal)?;
            journal.match_add(unit_field, unit_name)?;
            journal.seek_tail()?;
            let Some(record) = journal.previous_entry()? else {
                return Ok(None);
            };
            Ok(Some((record, journal.timestamp()?)))
        };
        let (record, at) = latest_entry()
            .inspect_err(|error| {
                tracing::warn!("coredump of {unit_name}: journal read failed: {error}");
            })
            .ok()??;
        let record = decode_record(record)
            .inspect_err(|error| {
                tracing::warn!("coredump of {unit_name}: malformed record: {error}");
            })
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

    fn firefox_coredump<'field>(
        varying_fields: impl IntoIterator<Item = (&'field str, &'field str)>,
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
                unit: CoredumpOwner::Unit(OwningUnit::User("app-firefox.scope".to_owned())),
                core_file: CoreFileStorage::NotOnDisk,
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
