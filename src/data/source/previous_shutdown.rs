use std::{
    cmp::Reverse,
    collections::HashMap,
    io,
    time::{Duration, SystemTime},
};

use systemd::{
    id128::Id128,
    journal::{JournalRecord, OpenOptions},
};

use crate::data::{
    severity::Severity,
    source::journal::{OwningUnit, either_of, field},
};

const SHUTDOWN_MESSAGE_ID: &str = "98268866d1d54a499c4e98921d93bc40";
const UNIT_STOPPING_MESSAGE_ID: &str = "de5b426a63be47a7b6ac3eaac82e2f6f";
const UNIT_STOPPED_MESSAGE_ID: &str = "9d1aaa27d60140bd96365438aad20286";
const SLOWEST_STOP_COUNT: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopOutcome {
    Finished {
        duration: Duration,
        job_result: String,
        severity: Severity,
    },
    Unfinished,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StopJob {
    unit: OwningUnit,
    job_id: String,
}

impl StopJob {
    fn of_record(record: &JournalRecord) -> Option<Self> {
        Some(Self {
            unit: OwningUnit::from_fields(record, "UNIT", "USER_UNIT")?,
            job_id: field(record, "JOB_ID")?.to_owned(),
        })
    }
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
        job_result: String,
    },
}

impl ShutdownEvent {
    fn of_record(record: &JournalRecord, at: SystemTime) -> Option<Self> {
        match field(record, "MESSAGE_ID")? {
            SHUTDOWN_MESSAGE_ID => Some(ShutdownEvent::Began { at }),
            UNIT_STOPPING_MESSAGE_ID => Some(ShutdownEvent::Stopping {
                job: StopJob::of_record(record)?,
                at,
            }),
            UNIT_STOPPED_MESSAGE_ID => Some(ShutdownEvent::Stopped {
                job: StopJob::of_record(record)?,
                at,
                job_result: field(record, "JOB_RESULT")?.to_owned(),
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

pub fn read_last_shutdown(slow_stop: Duration) -> io::Result<Option<LastShutdown>> {
    let mut journal = OpenOptions::default()
        .system(true)
        .current_user(true)
        .open()?;
    journal.match_add("_BOOT_ID", Id128::from_boot()?.to_string())?;
    journal.seek_head()?;
    if journal.next_entry()?.is_none() {
        return Ok(None);
    }
    journal.match_flush()?;
    let Some(last_of_previous_boot) = journal.previous_entry()? else {
        return Ok(None);
    };
    let ended_at = journal.timestamp()?;
    let Some(previous_boot) = field(&last_of_previous_boot, "_BOOT_ID") else {
        return Ok(None);
    };
    journal.match_add("_BOOT_ID", previous_boot.to_owned())?;
    for message_id in [
        SHUTDOWN_MESSAGE_ID,
        UNIT_STOPPING_MESSAGE_ID,
        UNIT_STOPPED_MESSAGE_ID,
    ] {
        journal.match_add("MESSAGE_ID", message_id)?;
    }
    journal.seek_head()?;
    let mut events = Vec::new();
    while let Some(record) = journal.next_entry()? {
        events.extend(ShutdownEvent::of_record(&record, journal.timestamp()?));
    }
    Ok(LastShutdown::assemble(events, ended_at, slow_stop))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_pair_by_job_and_a_stop_that_never_ended_ranks_first() {
        let at = |secs: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        let job = |unit: &str, job_id: &str| StopJob {
            unit: OwningUnit::System(unit.to_owned()),
            job_id: job_id.to_owned(),
        };
        let events = [
            ShutdownEvent::Stopping {
                job: job("restarted.service", "1"),
                at: at(5),
            },
            ShutdownEvent::Stopped {
                job: job("restarted.service", "1"),
                at: at(6),
                job_result: "done".to_owned(),
            },
            ShutdownEvent::Began { at: at(100) },
            ShutdownEvent::Stopping {
                job: job("nfs.mount", "7"),
                at: at(101),
            },
            ShutdownEvent::Stopping {
                job: job("docker.service", "8"),
                at: at(101),
            },
            ShutdownEvent::Stopped {
                job: job("docker.service", "8"),
                at: at(109),
                job_result: "done".to_owned(),
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
                        job_result: "done".to_owned(),
                        severity: Severity::Warning,
                    },
                },
            ]
        );
    }
}
