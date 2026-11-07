use std::{
    future::Future,
    io,
    time::{Duration, Instant, SystemTime},
};

use futures::{Stream, StreamExt};
use serde::Deserialize;
use systemd::journal::Journal;

use crate::{
    delivery_cadence_drivers::{PeriodicPolling, SamplingNeed, StampedCadence},
    podman_container_engine::libpod_running_containers::ENGINE_POLL_PERIOD,
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
    system_protocol_clients::journal_follow::{JournalEvent, journal_windows},
};

const PODMAN_TYPE_FIELD: &str = "PODMAN_TYPE";
const CONTAINER_PODMAN_TYPE: &str = "container";
const RUNNING_SET_EVENTS: [&str; 3] = ["start", "died", "remove"];
const TRIGGER_BURST_GAP: Duration = Duration::from_secs(1);

#[derive(Debug, Deserialize)]
pub(crate) struct PodmanEventRecord {
    #[serde(rename = "PODMAN_EVENT")]
    podman_event: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunningSetChange {
    at: SystemTime,
}

impl JournalEvent for RunningSetChange {
    type Record = PodmanEventRecord;

    const NAME: &'static str = "podman container event";
    const OPENS_USER_JOURNALS: bool = true;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        journal.match_add(PODMAN_TYPE_FIELD, CONTAINER_PODMAN_TYPE)?;
        Ok(())
    }

    fn admit(record: PodmanEventRecord, at: SystemTime) -> Option<Self> {
        RUNNING_SET_EVENTS
            .contains(&record.podman_event.as_str())
            .then_some(RunningSetChange { at })
    }

    fn at(&self) -> SystemTime {
        self.at
    }

    fn expires_at(&self) -> SystemTime {
        self.at
    }
}

fn running_set_triggers() -> impl Stream<Item = ()> + Send {
    journal_windows::<RunningSetChange>()
        .map(|window| {
            if let Err(failure) = window {
                tracing::warn!("podman events: {failure}, polling the engines instead");
            }
        })
        .chain(
            PeriodicPolling::every(ENGINE_POLL_PERIOD)
                .poll_ticks()
                .map(|_poll_tick| ()),
        )
        .at_most_once_per(TRIGGER_BURST_GAP)
}

pub(crate) struct ChangeTrackedSample<T> {
    pub sample: T,
    pub stamped_cadence: StampedCadence,
}

pub(crate) fn samples_on_running_set_changes<T, F>(
    read: impl FnMut() -> F + Send + 'static,
    sampling_need: fn(&T) -> SamplingNeed,
) -> impl Stream<Item = ChangeTrackedSample<T>> + Send
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let usage_recheck = PeriodicPolling::every(ENGINE_POLL_PERIOD);
    reread_on_trigger_or_deadline(running_set_triggers(), read, move |sample| {
        (sampling_need(sample) == SamplingNeed::KeepSampling)
            .then(|| Instant::now() + ENGINE_POLL_PERIOD)
    })
    .map(move |sample| ChangeTrackedSample {
        stamped_cadence: StampedCadence::pushed_with_recheck_while(
            sampling_need(&sample),
            usage_recheck,
        ),
        sample,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_protocol_clients::journal_follow::{decode_record, journal_record};

    fn admitted(podman_event: &str) -> Option<RunningSetChange> {
        let record = decode_record(journal_record([
            ("PODMAN_TYPE", "container"),
            ("PODMAN_EVENT", podman_event),
            ("PODMAN_NAME", "wp_01M6PNY1VE3AT8S4SG0F6W5QR7"),
        ]))
        .expect("podman event record decodes");
        RunningSetChange::admit(record, SystemTime::UNIX_EPOCH)
    }

    #[test]
    fn starts_deaths_and_removals_change_the_running_set() {
        for podman_event in ["start", "died", "remove"] {
            assert!(admitted(podman_event).is_some(), "{podman_event}");
        }
    }

    #[test]
    fn lifecycle_steps_around_them_are_ignored() {
        for podman_event in ["create", "init", "cleanup", "stop", "exec"] {
            assert!(admitted(podman_event).is_none(), "{podman_event}");
        }
    }
}
