use std::{collections::HashMap, io, time::SystemTime};

use systemd::journal::{Journal, JournalRecord};

use crate::data::{
    severity::Severity,
    source::journal::{JournalEvent, JournalPriority, OwningUnit, either_of, field},
};

const SERVICE_UNIT_SUFFIX: &str = ".service";

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
        JournalPriority::Error.match_this_urgent_or_more(journal)
    }

    fn from_record(record: &JournalRecord, at: SystemTime) -> Option<Self> {
        if field(record, "_TRANSPORT") == Some("kernel") {
            return None;
        }
        let unit = OwningUnit::from_fields(record, "_SYSTEMD_UNIT", "_SYSTEMD_USER_UNIT")
            .filter(|unit| unit.name().ends_with(SERVICE_UNIT_SUFFIX))?;
        Some(ServiceErrorEntry {
            at,
            unit,
            message: field(record, "MESSAGE")?.to_owned(),
            severity: JournalPriority::of_record(record)?.severity(),
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

    fn user_unit_error(user_unit: &str) -> JournalRecord {
        [
            ("PRIORITY", "3"),
            ("MESSAGE", "dial tcp: connection refused"),
            ("_TRANSPORT", "stdout"),
            ("_SYSTEMD_UNIT", "user@1000.service"),
            ("_SYSTEMD_USER_UNIT", user_unit),
        ]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .into()
    }

    #[test]
    fn an_error_logged_by_a_user_service_belongs_to_that_service_but_a_scope_is_no_service() {
        assert_eq!(
            ServiceErrorEntry::from_record(
                &user_unit_error("syncthing.service"),
                SystemTime::UNIX_EPOCH
            ),
            Some(ServiceErrorEntry {
                at: SystemTime::UNIX_EPOCH,
                unit: OwningUnit::User("syncthing.service".to_owned()),
                message: "dial tcp: connection refused".to_owned(),
                severity: Severity::Warning,
            })
        );
        assert_eq!(
            ServiceErrorEntry::from_record(
                &user_unit_error("kitty-1.scope"),
                SystemTime::UNIX_EPOCH
            ),
            None
        );
    }
}
