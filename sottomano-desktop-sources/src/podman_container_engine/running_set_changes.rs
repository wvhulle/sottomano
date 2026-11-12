use std::{
    future::{Future, ready},
    io,
    time::{Duration, SystemTime},
};

use futures::{Stream, StreamExt};
use serde::Deserialize;
use systemd::journal::Journal;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{
        PeriodicPolling, SamplingNeed, SnapshotDeliveryStreams, triggered_deliveries_with_recheck,
    },
    podman_container_engine::libpod_running_containers::ENGINE_POLL_PERIOD,
    system_protocol_clients::{
        journal_follow::{JournalEvent, journal_windows},
        protocol_peer_reconnects::subscribed,
    },
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

pub(crate) fn running_set_triggers() -> impl Stream<Item = ()> + Send {
    subscribed((), |()| async { Ok(journal_windows::<RunningSetChange>()) }).filter_map(|window| {
        if let Err(failure) = window {
            tracing::warn!("podman events: {failure}, reopening the journal");
            return ready(None);
        }
        ready(Some(()))
    })
}

pub(crate) fn deliveries_on_running_set_changes<Source, ReadFuture>(
    read: impl FnMut() -> ReadFuture + Send + 'static,
    sampling_need: fn(&Result<Source, Source::Error>) -> SamplingNeed,
) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
where
    Source: SnapshotSource + PartialEq,
    ReadFuture: Future<Output = Result<Source, Source::Error>> + Send + 'static,
{
    triggered_deliveries_with_recheck(
        running_set_triggers(),
        TRIGGER_BURST_GAP,
        PeriodicPolling::every(ENGINE_POLL_PERIOD),
        read,
        sampling_need,
    )
    .repeats_as_unchanged()
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
