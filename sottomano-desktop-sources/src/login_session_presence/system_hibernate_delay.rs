use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{Stream, future::ready};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeTriggerStreams, ChangeWatchFailure},
    system_protocol_clients::inotify_directory_changes::{WatchTarget, changes},
};

const MAIN_SLEEP_CONFIG: &str = "/etc/systemd/sleep.conf";
const DROP_IN_DIRECTORIES_BY_PRECEDENCE: [&str; 3] = [
    "/etc/systemd/sleep.conf.d",
    "/run/systemd/sleep.conf.d",
    "/usr/lib/systemd/sleep.conf.d",
];
const SLEEP_SECTION: &str = "Sleep";
const HIBERNATE_DELAY_KEY: &str = "HibernateDelaySec";
const DROP_IN_EXTENSION: &str = "conf";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HibernateDelay {
    Configured(Duration),
    BatteryEstimated,
}

#[derive(Debug, thiserror::Error)]
pub enum HibernateDelayReadError {
    #[error("{0}")]
    Watch(#[from] ChangeWatchFailure),
    #[error("reading sleep configuration: {0}")]
    Read(#[from] io::Error),
}

impl SnapshotSource for HibernateDelay {
    type Error = HibernateDelayReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Hibernate delay";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries((), |()| {
            let watched_paths = DROP_IN_DIRECTORIES_BY_PRECEDENCE
                .into_iter()
                .map(|directory| WatchTarget::EntriesOf(PathBuf::from(directory)))
                .chain([WatchTarget::File(PathBuf::from(MAIN_SLEEP_CONFIG))])
                .collect();
            ready(Ok(
                changes(watched_paths).triggered_rereads(|| ready(read_hibernate_delay()))
            ))
        })
    }
}

fn read_hibernate_delay() -> Result<HibernateDelay, HibernateDelayReadError> {
    sleep_config_files_in_order()?
        .iter()
        .map(|config_path| read_optional_text(config_path))
        .try_fold(HibernateDelay::BatteryEstimated, |delay, config_text| {
            Ok(config_text?.map_or(delay, |text| {
                last_hibernate_delay_in(&text).unwrap_or(delay)
            }))
        })
}

fn read_optional_text(config_path: &Path) -> Result<Option<String>, io::Error> {
    match fs::read_to_string(config_path) {
        Ok(text) => Ok(Some(text)),
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(read_failure) => Err(read_failure),
    }
}

fn sleep_config_files_in_order() -> Result<Vec<PathBuf>, io::Error> {
    let mut drop_ins_by_name: BTreeMap<OsString, PathBuf> = BTreeMap::new();
    for directory in DROP_IN_DIRECTORIES_BY_PRECEDENCE.iter().rev() {
        for drop_in_path in drop_in_files(Path::new(directory))? {
            if let Some(file_name) = drop_in_path.file_name() {
                drop_ins_by_name.insert(file_name.to_os_string(), drop_in_path);
            }
        }
    }
    Ok([PathBuf::from(MAIN_SLEEP_CONFIG)]
        .into_iter()
        .chain(drop_ins_by_name.into_values())
        .collect())
}

fn drop_in_files(directory: &Path) -> Result<Vec<PathBuf>, io::Error> {
    match fs::read_dir(directory) {
        Ok(entries) => Ok(entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == DROP_IN_EXTENSION)
            })
            .collect()),
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(read_failure) => Err(read_failure),
    }
}

fn last_hibernate_delay_in(config_text: &str) -> Option<HibernateDelay> {
    let mut current_section = "";
    let mut last_delay = None;
    for line in config_text.lines().map(str::trim) {
        if let Some(section_name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            current_section = section_name;
            continue;
        }
        if current_section != SLEEP_SECTION {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != HIBERNATE_DELAY_KEY {
            continue;
        }
        match parsed_hibernate_delay(value.trim()) {
            Ok(delay) => last_delay = Some(delay),
            Err(span_error) => {
                tracing::warn!(value = value.trim(), %span_error, "ignoring unparsable HibernateDelaySec");
            }
        }
    }
    last_delay
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TimeSpanError {
    #[error("missing amount before {0:?}")]
    MissingAmount(String),
    #[error("amount {0} is too large")]
    AmountTooLarge(String),
    #[error("unknown time unit {0:?}")]
    UnknownUnit(String),
}

fn parsed_hibernate_delay(value: &str) -> Result<HibernateDelay, TimeSpanError> {
    if value.is_empty() || value == "infinity" {
        return Ok(HibernateDelay::BatteryEstimated);
    }
    parsed_time_span(value).map(HibernateDelay::Configured)
}

fn parsed_time_span(value: &str) -> Result<Duration, TimeSpanError> {
    let mut total_span = Duration::ZERO;
    let mut remaining_text = value.trim();
    while !remaining_text.is_empty() {
        let digit_count = remaining_text
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(remaining_text.len());
        let amount_text = &remaining_text[..digit_count];
        if amount_text.is_empty() {
            return Err(TimeSpanError::MissingAmount(remaining_text.to_owned()));
        }
        let too_large = || TimeSpanError::AmountTooLarge(amount_text.to_owned());
        let amount: u32 = amount_text.parse().map_err(|_| too_large())?;
        let after_amount = remaining_text[digit_count..].trim_start();
        let unit_length = after_amount
            .find(|character: char| character.is_ascii_digit() || character.is_whitespace())
            .unwrap_or(after_amount.len());
        let unit_span = time_unit_span(&after_amount[..unit_length])?;
        total_span += unit_span.checked_mul(amount).ok_or_else(too_large)?;
        remaining_text = after_amount[unit_length..].trim_start();
    }
    Ok(total_span)
}

fn time_unit_span(unit_name: &str) -> Result<Duration, TimeSpanError> {
    match unit_name {
        "us" | "usec" | "µs" => Ok(Duration::from_micros(1)),
        "ms" | "msec" => Ok(Duration::from_millis(1)),
        "" | "s" | "sec" | "second" | "seconds" => Ok(Duration::from_secs(1)),
        "m" | "min" | "minute" | "minutes" => Ok(Duration::from_mins(1)),
        "h" | "hr" | "hour" | "hours" => Ok(Duration::from_hours(1)),
        "d" | "day" | "days" => Ok(Duration::from_hours(24)),
        "w" | "week" | "weeks" => Ok(Duration::from_hours(24 * 7)),
        _ => Err(TimeSpanError::UnknownUnit(unit_name.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{HibernateDelay, TimeSpanError, last_hibernate_delay_in, parsed_time_span};

    #[test]
    fn systemd_time_spans_add_their_units() {
        assert_eq!(parsed_time_span("3h"), Ok(Duration::from_hours(3)));
        assert_eq!(parsed_time_span("1h 30min"), Ok(Duration::from_mins(90)));
        assert_eq!(parsed_time_span("5400"), Ok(Duration::from_mins(90)));
        assert_eq!(parsed_time_span("2 d"), Ok(Duration::from_hours(48)));
        assert_eq!(
            parsed_time_span("3 fortnights"),
            Err(TimeSpanError::UnknownUnit("fortnights".to_owned()))
        );
    }

    #[test]
    fn only_the_sleep_section_sets_the_delay_and_the_last_assignment_wins() {
        let config_text = "[Other]\nHibernateDelaySec=1h\n[Sleep]\nHibernateDelaySec=2h\\
                           nSuspendState=mem\nHibernateDelaySec=3h\n";
        assert_eq!(
            last_hibernate_delay_in(config_text),
            Some(HibernateDelay::Configured(Duration::from_hours(3)))
        );
    }

    #[test]
    fn an_empty_assignment_returns_to_battery_estimation() {
        assert_eq!(
            last_hibernate_delay_in("[Sleep]\nHibernateDelaySec=3h\nHibernateDelaySec=\n"),
            Some(HibernateDelay::BatteryEstimated)
        );
    }

    #[test]
    fn an_unparsable_assignment_keeps_the_earlier_value_like_systemd() {
        assert_eq!(
            last_hibernate_delay_in("[Sleep]\nHibernateDelaySec=2h\nHibernateDelaySec=soon\n"),
            Some(HibernateDelay::Configured(Duration::from_hours(2)))
        );
    }

    #[test]
    fn a_file_without_the_key_leaves_the_earlier_value() {
        assert_eq!(last_hibernate_delay_in("[Sleep]\nSuspendState=mem\n"), None);
    }
}
