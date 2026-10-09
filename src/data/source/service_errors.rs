use std::{collections::HashMap, io, time::SystemTime};

use systemd::journal::{Journal, JournalRecord};

use crate::data::{
    severity::Severity,
    source::journal::{JournalEvent, OwningUnit, field},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceErrorEntry {
    pub at: SystemTime,
    pub unit: OwningUnit,
    pub message: String,
    pub severity: Severity,
}

impl JournalEvent for ServiceErrorEntry {
    const NAME: &'static str = "service error";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        Severity::Critical.match_journal_priorities(journal)
    }

    fn from_record(record: &JournalRecord, at: SystemTime) -> Option<Self> {
        if field(record, "_TRANSPORT") == Some("kernel") {
            return None;
        }
        Some(ServiceErrorEntry {
            at,
            unit: OwningUnit::from_fields(record, "_SYSTEMD_UNIT", "_SYSTEMD_USER_UNIT")?,
            message: field(record, "MESSAGE")?.to_owned(),
            severity: Severity::of_journal_record(record)?,
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
    pub entries: Vec<ServiceErrorEntry>,
}

impl ServiceErrors {
    pub fn from_entries(entries: Vec<ServiceErrorEntry>) -> Option<Self> {
        (!entries.is_empty()).then_some(Self { entries })
    }

    pub fn by_unit(&self) -> Vec<UnitErrors> {
        let mut by_unit: HashMap<&OwningUnit, UnitErrors> = HashMap::new();
        for entry in &self.entries {
            by_unit
                .entry(&entry.unit)
                .and_modify(|errors| {
                    errors.count += 1;
                    errors.severity = errors.severity.max(entry.severity);
                    if entry.at >= errors.latest {
                        errors.latest = entry.at;
                        errors.latest_message.clone_from(&entry.message);
                    }
                })
                .or_insert_with(|| UnitErrors {
                    unit: entry.unit.clone(),
                    latest: entry.at,
                    latest_message: entry.message.clone(),
                    count: 1,
                    severity: entry.severity,
                });
        }
        let mut units: Vec<_> = by_unit.into_values().collect();
        units.sort_by_key(|errors| errors.latest);
        units
    }

    pub fn follow_log_args(&self) -> Vec<String> {
        let mut units: Vec<&OwningUnit> = self.entries.iter().map(|entry| &entry.unit).collect();
        units.sort();
        units.dedup();
        ["-b", "-f", "-n", "all", "-p", "err"]
            .map(String::from)
            .into_iter()
            .chain(units.into_iter().enumerate().flat_map(|(index, unit)| {
                (index > 0)
                    .then(|| String::from("+"))
                    .into_iter()
                    .chain([unit.logged_by_match()])
            }))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_logged_by_a_user_service_belongs_to_that_service() {
        let record: JournalRecord = [
            ("PRIORITY", "3"),
            ("MESSAGE", "dial tcp: connection refused"),
            ("_TRANSPORT", "stdout"),
            ("_SYSTEMD_UNIT", "user@1000.service"),
            ("_SYSTEMD_USER_UNIT", "syncthing.service"),
        ]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .into();
        assert_eq!(
            ServiceErrorEntry::from_record(&record, SystemTime::UNIX_EPOCH),
            Some(ServiceErrorEntry {
                at: SystemTime::UNIX_EPOCH,
                unit: OwningUnit::User("syncthing.service".to_owned()),
                message: "dial tcp: connection refused".to_owned(),
                severity: Severity::Critical,
            })
        );
    }
}
