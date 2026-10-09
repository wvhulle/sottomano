use std::{collections::HashSet, io, time::SystemTime};

use systemd::journal::{Journal, JournalRecord};

use crate::data::{
    severity::Severity,
    source::journal::{JournalEvent, JournalPriority, OwningUnit, field},
};

const KERNEL_OOM_KILL_MESSAGE_ID: &str = "fe6faa94e7774663a0da52717891d8ef";
const OOMD_KILL_MESSAGE_ID: &str = "d989611b15e44c9dbf31e3c81256e4ed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OomKiller {
    Kernel,
    Oomd,
}

impl OomKiller {
    fn of_message_id(message_id: &str) -> Option<Self> {
        match message_id {
            KERNEL_OOM_KILL_MESSAGE_ID => Some(OomKiller::Kernel),
            OOMD_KILL_MESSAGE_ID => Some(OomKiller::Oomd),
            _ => None,
        }
    }
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
    const NAME: &'static str = "oom kill";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        journal.match_add("MESSAGE_ID", KERNEL_OOM_KILL_MESSAGE_ID)?;
        journal.match_add("MESSAGE_ID", OOMD_KILL_MESSAGE_ID)?;
        Ok(())
    }

    fn from_record(record: &JournalRecord, at: SystemTime) -> Option<Self> {
        Some(OomKillEntry {
            at,
            unit: OwningUnit::from_fields(record, "UNIT", "USER_UNIT")?,
            killer: OomKiller::of_message_id(field(record, "MESSAGE_ID")?)?,
            reporting_manager_unit: field(record, "_SYSTEMD_UNIT")?.to_owned(),
            severity: JournalPriority::of_record(record)?.severity(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

#[derive(Debug, Clone)]
pub struct OomKills {
    pub entries: Vec<OomKillEntry>,
}

impl OomKills {
    pub fn from_entries(entries: Vec<OomKillEntry>) -> Option<Self> {
        let user_managers: HashSet<String> = entries
            .iter()
            .filter(|entry| matches!(entry.unit, OwningUnit::User(_)))
            .map(|entry| entry.reporting_manager_unit.clone())
            .collect();
        let innermost: Vec<OomKillEntry> = entries
            .into_iter()
            .filter(|entry| match &entry.unit {
                OwningUnit::System(name) => !user_managers.contains(name),
                OwningUnit::User(_) => true,
            })
            .collect();
        (!innermost.is_empty()).then_some(Self { entries: innermost })
    }

    pub fn follow_log_args() -> Vec<String> {
        ["-b", "-f", "-n", "all"]
            .map(String::from)
            .into_iter()
            .chain(
                [KERNEL_OOM_KILL_MESSAGE_ID, OOMD_KILL_MESSAGE_ID]
                    .map(|message_id| format!("MESSAGE_ID={message_id}")),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kill_record(fields: [(&str, &str); 3]) -> OomKillEntry {
        let record: JournalRecord = fields
            .into_iter()
            .chain([("PRIORITY", "5")])
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect();
        OomKillEntry::from_record(&record, SystemTime::UNIX_EPOCH).expect("an oom kill record")
    }

    #[test]
    fn a_user_unit_oom_kill_hides_its_user_manager_echo_from_pid1() {
        let from_pid1 = kill_record([
            ("MESSAGE_ID", KERNEL_OOM_KILL_MESSAGE_ID),
            ("UNIT", "user@1000.service"),
            ("_SYSTEMD_UNIT", "init.scope"),
        ]);
        let from_user_manager = kill_record([
            ("MESSAGE_ID", KERNEL_OOM_KILL_MESSAGE_ID),
            ("USER_UNIT", "run-p42.service"),
            ("_SYSTEMD_UNIT", "user@1000.service"),
        ]);
        let kills = OomKills::from_entries(vec![from_pid1, from_user_manager.clone()])
            .expect("one kill remains");
        assert_eq!(kills.entries, [from_user_manager]);
    }
}
