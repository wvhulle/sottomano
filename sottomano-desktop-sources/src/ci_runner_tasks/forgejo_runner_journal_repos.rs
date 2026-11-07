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
    pub(super) instance_url: Option<String>,
}

struct ParsedTaskStart<'message> {
    task_id: u64,
    repository: &'message str,
    instance_url: Option<&'message str>,
}

fn parse_task_start_message(message: &str) -> Option<ParsedTaskStart<'_>> {
    let (_, after_task) = message.split_once("task ")?;
    let (task_id, after_repo) = after_task.split_once(" repo is ")?;
    let mut repo_words = after_repo.split_whitespace();
    let repository = repo_words.next()?;
    let instance_url = repo_words
        .map(|word| word.trim_end_matches('"'))
        .rfind(|word| word.starts_with("http"));
    Some(ParsedTaskStart {
        task_id: task_id.parse().ok()?,
        repository,
        instance_url,
    })
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
        let parsed = parse_task_start_message(&record.message)?;
        Some(ForgejoTaskStart {
            at,
            task_id: parsed.task_id,
            repository: parsed.repository.to_owned(),
            instance_url: parsed.instance_url.map(str::to_owned),
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
    fn the_runner_log_line_yields_task_repo_and_instance() {
        let message = "time=\"2026-11-05T19:03:55+01:00\" level=info msg=\"task 11282424 repo is wvhulle/bindrune https://data.forgejo.org https://codeberg.org/\"";
        let parsed = parse_task_start_message(message).expect("the line is a task start");
        assert_eq!(parsed.task_id, 11_282_424);
        assert_eq!(parsed.repository, "wvhulle/bindrune");
        assert_eq!(parsed.instance_url, Some("https://codeberg.org/"));
    }

    #[test]
    fn a_line_without_an_instance_still_names_the_repo() {
        let parsed = parse_task_start_message("task 7 repo is wvhulle/bindrune")
            .expect("the line is a task start");
        assert_eq!(parsed.repository, "wvhulle/bindrune");
        assert_eq!(parsed.instance_url, None);
    }

    #[test]
    fn other_runner_log_lines_are_not_task_starts() {
        assert!(parse_task_start_message("failed to fetch task: deadline exceeded").is_none());
        assert!(parse_task_start_message("[poller] launched").is_none());
    }
}
