use std::{
    cmp::Reverse,
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    time::{Duration, SystemTime},
};

use futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use sottomano_config::severity::Severity;
use systemd::{id128::Id128, journal::OpenOptions};

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    system_protocol_clients::journal_follow::decode_record,
    systemd_units_journal::journal_record_fields::{MessageId, OwningUnit, SubjectUnit, either_of},
};

const SLOWEST_STOP_COUNT: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopOutcome {
    Finished {
        duration: Duration,
        job_result: JobResult,
        severity: Severity,
    },
    Unfinished,
}

impl StopOutcome {
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self {
            StopOutcome::Unfinished => Severity::Warning,
            StopOutcome::Finished { severity, .. } => *severity,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum StopRank {
    Unfinished,
    Finished(Reverse<Duration>),
}

impl StopOutcome {
    fn rank(&self) -> StopRank {
        match self {
            StopOutcome::Unfinished => StopRank::Unfinished,
            StopOutcome::Finished { duration, .. } => StopRank::Finished(Reverse(*duration)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitStop {
    pub unit: OwningUnit,
    pub outcome: StopOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastShutdown {
    pub duration: Duration,
    pub slowest_stops: Vec<UnitStop>,
}

impl LastShutdown {
    pub fn follow_log_args(&self) -> Vec<String> {
        ["-b", "-1", "-x", "-n", "all"]
            .map(String::from)
            .into_iter()
            .chain(either_of(
                self.slowest_stops
                    .iter()
                    .map(|stop| stop.unit.logged_by_match()),
            ))
            .collect()
    }

    fn assemble(
        events: impl IntoIterator<Item = ShutdownEvent>,
        ended_at: SystemTime,
        slow_stop: Duration,
    ) -> Option<Self> {
        let events: Vec<ShutdownEvent> = events.into_iter().collect();
        let began_at = events.iter().find_map(ShutdownEvent::shutdown_began_at)?;
        let mut stopping_since: HashMap<StopJob, SystemTime> = HashMap::new();
        let mut stops = Vec::new();
        for event in events.into_iter().filter(|event| event.at() >= began_at) {
            match event {
                ShutdownEvent::Began { .. } => {}
                ShutdownEvent::Stopping { job, at } => {
                    stopping_since.insert(job, at);
                }
                ShutdownEvent::Stopped {
                    job,
                    at,
                    job_result,
                } => {
                    let stop_duration = stopping_since
                        .remove(&job)
                        .and_then(|since| at.duration_since(since).ok());
                    if let Some(duration) = stop_duration {
                        stops.push(UnitStop {
                            unit: job.unit,
                            outcome: StopOutcome::Finished {
                                duration,
                                job_result,
                                severity: if duration >= slow_stop {
                                    Severity::Warning
                                } else {
                                    Severity::Normal
                                },
                            },
                        });
                    }
                }
            }
        }
        stops.extend(stopping_since.into_keys().map(|job| UnitStop {
            unit: job.unit,
            outcome: StopOutcome::Unfinished,
        }));
        stops.sort_by_key(|stop| stop.outcome.rank());
        stops.truncate(SLOWEST_STOP_COUNT);
        Some(Self {
            duration: ended_at.duration_since(began_at).ok()?,
            slowest_stops: stops,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobResult {
    Done,
    Canceled,
    Timeout,
    Failed,
    Dependency,
    Skipped,
    Invalid,
    Assert,
    Unsupported,
    Collected,
    Once,
    Frozen,
    Concurrency,
}

impl JobResult {
    fn label(self) -> &'static str {
        match self {
            JobResult::Done => "done",
            JobResult::Canceled => "canceled",
            JobResult::Timeout => "timeout",
            JobResult::Failed => "failed",
            JobResult::Dependency => "dependency",
            JobResult::Skipped => "skipped",
            JobResult::Invalid => "invalid",
            JobResult::Assert => "assert",
            JobResult::Unsupported => "unsupported",
            JobResult::Collected => "collected",
            JobResult::Once => "once",
            JobResult::Frozen => "frozen",
            JobResult::Concurrency => "concurrency",
        }
    }
}

impl Display for JobResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StopJob {
    unit: OwningUnit,
    job_id: u32,
}

#[derive(Debug, Deserialize)]
struct ShutdownRecord {
    #[serde(rename = "MESSAGE_ID")]
    message_id: MessageId,
    #[serde(flatten)]
    unit: SubjectUnit,
    #[serde(rename = "JOB_ID")]
    job_id: Option<u32>,
    #[serde(rename = "JOB_RESULT")]
    job_result: Option<JobResult>,
}

#[derive(Debug, Deserialize)]
struct BootRecord {
    #[serde(rename = "_BOOT_ID")]
    boot_id: String,
}

enum ShutdownEvent {
    Began {
        at: SystemTime,
    },
    Stopping {
        job: StopJob,
        at: SystemTime,
    },
    Stopped {
        job: StopJob,
        at: SystemTime,
        job_result: JobResult,
    },
}

impl ShutdownEvent {
    const MESSAGE_IDS: [MessageId; 3] = [
        MessageId::SHUTDOWN,
        MessageId::UNIT_STOPPING,
        MessageId::UNIT_STOPPED,
    ];

    fn of_record(record: ShutdownRecord, at: SystemTime) -> Option<Self> {
        let ShutdownRecord {
            message_id,
            unit,
            job_id,
            job_result,
        } = record;
        let job = || {
            Some(StopJob {
                unit: unit.owning_unit()?,
                job_id: job_id?,
            })
        };
        match message_id {
            MessageId::SHUTDOWN => Some(ShutdownEvent::Began { at }),
            MessageId::UNIT_STOPPING => Some(ShutdownEvent::Stopping { job: job()?, at }),
            MessageId::UNIT_STOPPED => Some(ShutdownEvent::Stopped {
                job: job()?,
                at,
                job_result: job_result?,
            }),
            _ => None,
        }
    }

    fn at(&self) -> SystemTime {
        match self {
            ShutdownEvent::Began { at }
            | ShutdownEvent::Stopping { at, .. }
            | ShutdownEvent::Stopped { at, .. } => *at,
        }
    }

    fn shutdown_began_at(&self) -> Option<SystemTime> {
        match self {
            ShutdownEvent::Began { at } => Some(*at),
            ShutdownEvent::Stopping { .. } | ShutdownEvent::Stopped { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownReadError {
    Journal(io::ErrorKind),
    NoPreviousBoot,
    NoShutdownRecorded,
    Panicked,
}

impl From<io::Error> for ShutdownReadError {
    fn from(error: io::Error) -> Self {
        Self::Journal(error.kind())
    }
}

impl From<Panicked> for ShutdownReadError {
    fn from(Panicked: Panicked) -> Self {
        Self::Panicked
    }
}

impl Display for ShutdownReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Journal(kind) => write!(f, "cannot read the journal: {kind}"),
            Self::NoPreviousBoot => write!(f, "no previous boot in the journal"),
            Self::NoShutdownRecorded => write!(f, "previous boot recorded no shutdown"),
            Self::Panicked => write!(f, "shutdown read panicked"),
        }
    }
}

impl SnapshotSource for LastShutdown {
    type Error = ShutdownReadError;
    type StreamInputs = Duration;

    const NAME: &'static str = "last shutdown";

    fn delivery_stream(slow_stop: Duration) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(blocking_poll(move || read_last_shutdown(slow_stop))).map(|read| match read {
            Err(ShutdownReadError::NoPreviousBoot | ShutdownReadError::NoShutdownRecorded) => {
                SnapshotDelivery::SourceUnavailable
            }
            read => SnapshotDelivery::live_push(read),
        })
    }
}

fn read_last_shutdown(slow_stop: Duration) -> Result<LastShutdown, ShutdownReadError> {
    let mut journal = OpenOptions::default()
        .system(true)
        .current_user(true)
        .open()?;
    journal.match_add("_BOOT_ID", Id128::from_boot()?.to_string())?;
    journal.seek_head()?;
    journal
        .next_entry()?
        .ok_or(ShutdownReadError::NoPreviousBoot)?;
    journal.match_flush()?;
    let last_of_previous_boot = journal
        .previous_entry()?
        .ok_or(ShutdownReadError::NoPreviousBoot)?;
    let ended_at = journal.timestamp()?;
    let previous_boot: BootRecord =
        decode_record(last_of_previous_boot).map_err(|_| ShutdownReadError::NoPreviousBoot)?;
    journal.match_add("_BOOT_ID", previous_boot.boot_id)?;
    for message_id in ShutdownEvent::MESSAGE_IDS {
        message_id.add_match(&mut journal)?;
    }
    journal.seek_head()?;
    let mut events = Vec::new();
    while let Some(record) = journal.next_entry()? {
        let at = journal.timestamp()?;
        let decoded = decode_record(record)
            .inspect_err(|e| tracing::debug!("systemd last shutdown: malformed record: {e}"))
            .ok();
        events.extend(decoded.and_then(|record| ShutdownEvent::of_record(record, at)));
    }
    LastShutdown::assemble(events, ended_at, slow_stop).ok_or(ShutdownReadError::NoShutdownRecorded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_protocol_clients::journal_follow::journal_record;

    #[test]
    fn stops_pair_by_job_and_a_stop_that_never_ended_ranks_first() {
        let at = |secs: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        let job = |unit: &str, job_id: u32| StopJob {
            unit: OwningUnit::System(unit.to_owned()),
            job_id,
        };
        let events = [
            ShutdownEvent::Stopping {
                job: job("restarted.service", 1),
                at: at(5),
            },
            ShutdownEvent::Stopped {
                job: job("restarted.service", 1),
                at: at(6),
                job_result: JobResult::Done,
            },
            ShutdownEvent::Began { at: at(100) },
            ShutdownEvent::Stopping {
                job: job("nfs.mount", 7),
                at: at(101),
            },
            ShutdownEvent::Stopping {
                job: job("docker.service", 8),
                at: at(101),
            },
            ShutdownEvent::Stopped {
                job: job("docker.service", 8),
                at: at(109),
                job_result: JobResult::Done,
            },
        ];
        let shutdown =
            LastShutdown::assemble(events, at(190), Duration::from_secs(5)).expect("a shutdown");
        assert_eq!(shutdown.duration, Duration::from_secs(90));
        assert_eq!(
            shutdown.slowest_stops,
            [
                UnitStop {
                    unit: OwningUnit::System("nfs.mount".to_owned()),
                    outcome: StopOutcome::Unfinished,
                },
                UnitStop {
                    unit: OwningUnit::System("docker.service".to_owned()),
                    outcome: StopOutcome::Finished {
                        duration: Duration::from_secs(8),
                        job_result: JobResult::Done,
                        severity: Severity::Warning,
                    },
                },
            ]
        );
    }

    #[test]
    fn a_stopped_unit_record_decodes_its_job_and_result() {
        let record = decode_record(journal_record([
            ("MESSAGE_ID", "9d1aaa27d60140bd96365438aad20286"),
            ("UNIT", "docker.service"),
            ("JOB_ID", "8"),
            ("JOB_RESULT", "timeout"),
        ]))
        .expect("a well-formed stop record");
        let Some(ShutdownEvent::Stopped {
            job, job_result, ..
        }) = ShutdownEvent::of_record(record, SystemTime::UNIX_EPOCH)
        else {
            panic!("a stopped unit event");
        };
        assert_eq!(
            job,
            StopJob {
                unit: OwningUnit::System("docker.service".to_owned()),
                job_id: 8,
            }
        );
        assert_eq!(job_result, JobResult::Timeout);
    }
}
