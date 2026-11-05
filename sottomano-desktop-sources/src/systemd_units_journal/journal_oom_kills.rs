use std::{
    collections::HashSet,
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    time::SystemTime,
};

use futures::{Stream, StreamExt};
use serde::Deserialize;
use sottomano_toml_config::severity::Severity;
use systemd::journal::Journal;

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::journal_follow::{JournalEvent, JournalFollowError, journal_windows},
    systemd_units_journal::journal_record_fields::{
        JournalPriority, MessageId, OwningUnit, SubjectUnit,
    },
};

impl SnapshotSource for Vec<OomKillEntry> {
    type Error = JournalFollowError;
    type StreamInputs = ();

    const NAME: &'static str = "OOM kills";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        journal_windows::<OomKillEntry>()
            .map(|entries| entries.map(OomKillEntry::innermost))
            .map(SnapshotDelivery::live_push)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "MessageId")]
pub enum OomKiller {
    Kernel,
    Oomd,
}

impl OomKiller {
    const ALL: [Self; 2] = [OomKiller::Kernel, OomKiller::Oomd];

    fn message_id(self) -> MessageId {
        match self {
            OomKiller::Kernel => MessageId::KERNEL_OOM_KILL,
            OomKiller::Oomd => MessageId::OOMD_KILL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAnOomKill(MessageId);

impl Display for NotAnOomKill {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "message id {} reports no oom kill", self.0)
    }
}

impl TryFrom<MessageId> for OomKiller {
    type Error = NotAnOomKill;

    fn try_from(message_id: MessageId) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|killer| killer.message_id() == message_id)
            .ok_or(NotAnOomKill(message_id))
    }
}

#[derive(Debug, Deserialize)]
pub struct OomKillRecord {
    #[serde(rename = "MESSAGE_ID")]
    killer: OomKiller,
    #[serde(flatten)]
    unit: SubjectUnit,
    #[serde(rename = "_SYSTEMD_UNIT")]
    reporting_manager_unit: String,
    #[serde(rename = "PRIORITY")]
    priority: JournalPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomKillEntry {
    pub at: SystemTime,
    pub unit: OwningUnit,
    pub killer: OomKiller,
    pub reporting_manager_unit: String,
    pub severity: Severity,
}

impl JournalEvent for OomKillEntry {
    type Record = OomKillRecord;

    const NAME: &'static str = "oom kill";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        OomKiller::ALL
            .into_iter()
            .try_for_each(|killer| killer.message_id().add_match(journal))
    }

    fn admit(record: OomKillRecord, at: SystemTime) -> Option<Self> {
        Some(OomKillEntry {
            at,
            unit: record.unit.owning_unit()?,
            killer: record.killer,
            reporting_manager_unit: record.reporting_manager_unit,
            severity: record.priority.severity(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

impl OomKillEntry {
    #[must_use]
    pub fn innermost(entries: Vec<Self>) -> Vec<Self> {
        let user_managers: HashSet<String> = entries
            .iter()
            .filter(|entry| matches!(entry.unit, OwningUnit::User(_)))
            .map(|entry| entry.reporting_manager_unit.clone())
            .collect();
        entries
            .into_iter()
            .filter(|entry| match &entry.unit {
                OwningUnit::System(name) => !user_managers.contains(name),
                OwningUnit::User(_) => true,
            })
            .collect()
    }

    pub fn follow_log_args() -> Vec<String> {
        ["-b", "-f", "-n", "all"]
            .map(String::from)
            .into_iter()
            .chain(OomKiller::ALL.map(|killer| killer.message_id().follow_match()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_protocol_clients::journal_follow::{decode_record, journal_record};

    const KERNEL_OOM_KILL: &str = "fe6faa94e7774663a0da52717891d8ef";

    fn kill_record(fields: [(&str, &str); 3]) -> OomKillEntry {
        let record = decode_record(journal_record(
            fields.into_iter().chain([("PRIORITY", "5")]),
        ))
        .expect("a well-formed oom kill record");
        OomKillEntry::admit(record, SystemTime::UNIX_EPOCH).expect("an oom kill of a unit")
    }

    #[test]
    fn a_user_unit_oom_kill_hides_its_user_manager_echo_from_pid1() {
        let from_pid1 = kill_record([
            ("MESSAGE_ID", KERNEL_OOM_KILL),
            ("UNIT", "user@1000.service"),
            ("_SYSTEMD_UNIT", "init.scope"),
        ]);
        let from_user_manager = kill_record([
            ("MESSAGE_ID", KERNEL_OOM_KILL),
            ("USER_UNIT", "run-p42.service"),
            ("_SYSTEMD_UNIT", "user@1000.service"),
        ]);
        let kills = OomKillEntry::innermost(vec![from_pid1, from_user_manager.clone()]);
        assert_eq!(kills, [from_user_manager]);
    }
}
