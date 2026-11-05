use std::{io, time::SystemTime};

use serde::Deserialize;
use systemd::journal::Journal;

use crate::{
    system_protocol_clients::journal_follow::JournalEvent,
    systemd_units_journal::journal_record_fields::LoggingUnit,
};

const SYSLOG_IDENTIFIER_FIELD: &str = "SYSLOG_IDENTIFIER";
const RUNNER_SYSLOG_IDENTIFIER: &str = "forgejo-runner";

#[derive(Debug, Deserialize)]
pub struct ForgejoTaskStartRecord {
    #[serde(rename = "MESSAGE")]
    message: String,
    #[serde(flatten)]
    unit: LoggingUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgejoTaskStart {
    at: SystemTime,
    pub(super) unit_name: String,
    pub(super) task_id: u64,
    pub(super) repository: String,
}

fn parse_task_start_message(message: &str) -> Option<(u64, &str)> {
    let (_, after_task) = message.split_once("task ")?;
    let (task_id, after_repo) = after_task.split_once(" repo is ")?;
    let repository = after_repo.split_whitespace().next()?;
    Some((task_id.parse().ok()?, repository))
}

impl JournalEvent for ForgejoTaskStart {
    type Record = ForgejoTaskStartRecord;

    const NAME: &'static str = "forgejo task start";
    const OPENS_USER_JOURNALS: bool = false;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        journal.match_add(SYSLOG_IDENTIFIER_FIELD, RUNNER_SYSLOG_IDENTIFIER)?;
        Ok(())
    }

    fn admit(record: ForgejoTaskStartRecord, at: SystemTime) -> Option<Self> {
        let (task_id, repository) = parse_task_start_message(&record.message)?;
        Some(ForgejoTaskStart {
            at,
            task_id,
            repository: repository.to_owned(),
            unit_name: record.unit.owning_unit()?.name().to_owned(),
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runner_log_line_yields_task_and_repo() {
        let message = "time=\"2026-11-05T19:03:55+01:00\" level=info msg=\"task 11282424 repo is wvhulle/bindrune https://data.forgejo.org https://codeberg.org/\"";
        assert_eq!(
            parse_task_start_message(message),
            Some((11_282_424, "wvhulle/bindrune"))
        );
    }

    #[test]
    fn other_runner_log_lines_are_not_task_starts() {
        assert_eq!(
            parse_task_start_message("failed to fetch task: deadline exceeded"),
            None
        );
        assert_eq!(parse_task_start_message("[poller] launched"), None);
    }
}
