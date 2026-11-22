pub(crate) mod libpod_engine_sampler;
mod libpod_listed_containers;
#[cfg(test)]
mod libpod_running_containers_tests;

use std::{
    cmp::Reverse,
    fmt::{Display, Formatter, Result as FmtResult},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytesize::ByteSize;
use chrono::DateTime;
use futures::{Stream, StreamExt};
use libpod_engine_sampler::EngineSampler;
use tokio::net::UnixStream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::Panicked,
    ci_runner_tasks::forgejo_runner_container_names::parse_forgejo_container_name,
    delivery_cadence_drivers::{CarriedRead, SamplingNeed},
    podman_container_engine::{
        libpod_inspect_ci_identity::WoodpeckerStepIdentity,
        running_set_changes::deliveries_on_running_set_changes,
    },
    system_protocol_clients::{
        http_json_client,
        journal_follow::JournalFollowError,
        procfs_process_tree::{ProcessTreeCpuLoad, total_cpu_load},
    },
};

pub const LIBPOD_API_ROOT: &str = "http://podman/v5.0.0/libpod";
pub(crate) const ENGINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const WOODPECKER_STEP_LABEL: &str = "wp_step";
const WOODPECKER_LABELS: [&str; 2] = ["wp_uuid", WOODPECKER_STEP_LABEL];

#[derive(Debug, Clone, PartialEq)]
pub enum EngineReadError {
    SocketMissing,
    SocketDenied,
    Fetch(http_json_client::Error),
    ProcessTableUnreadable,
    ReadPanicked,
    RunningSetUnwatched(JournalFollowError),
}

impl From<JournalFollowError> for EngineReadError {
    fn from(watch_failure: JournalFollowError) -> Self {
        EngineReadError::RunningSetUnwatched(watch_failure)
    }
}

pub(crate) fn unavailable_when_socket_missing<Source>(
    delivery: SnapshotDelivery<Source>,
) -> SnapshotDelivery<Source>
where
    Source: SnapshotSource<Error = EngineReadError>,
{
    delivery.unavailable_when(|read_failure| *read_failure == EngineReadError::SocketMissing)
}

impl From<http_json_client::Error> for EngineReadError {
    fn from(fetch_failure: http_json_client::Error) -> Self {
        EngineReadError::Fetch(fetch_failure)
    }
}

impl Display for EngineReadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            EngineReadError::SocketMissing => {
                formatter.write_str("the engine socket does not exist")
            }
            EngineReadError::SocketDenied => {
                formatter.write_str("this user may not open the engine socket")
            }
            EngineReadError::Fetch(failure) => failure.fmt(formatter),
            EngineReadError::ProcessTableUnreadable => formatter.write_str("cannot read /proc"),
            EngineReadError::ReadPanicked => Panicked.fmt(formatter),
            EngineReadError::RunningSetUnwatched(watch_failure) => watch_failure.fmt(formatter),
        }
    }
}

impl From<Panicked> for EngineReadError {
    fn from(Panicked: Panicked) -> Self {
        EngineReadError::ReadPanicked
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerCiIdentity {
    NotWoodpecker,
    WoodpeckerStep(WoodpeckerStepIdentity),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunningContainer {
    pub id: String,
    pub container_name: String,
    pub shown_name: String,
    pub image: String,
    pub started_at: SystemTime,
    pub ci_identity: ContainerCiIdentity,
    pub cpu_load: ProcessTreeCpuLoad,
    pub resident_memory: ByteSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineContainers {
    pub containers: Vec<RunningContainer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoppedContainer {
    pub id: String,
    pub container_name: String,
    pub shown_name: String,
    pub image: String,
    pub stopped_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineListing {
    pub running: EngineContainers,
    pub stopped: Vec<StoppedContainer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerSelection {
    Running,
    All,
}

impl ContainerSelection {
    pub(crate) fn list_query(self) -> &'static str {
        match self {
            ContainerSelection::Running => "",
            ContainerSelection::All => "?all=true",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerPhase {
    Running,
    Reusable,
    CiLeftover,
    Transitional,
}

pub(crate) fn created_by_ci_runner<'labels>(
    mut label_keys: impl Iterator<Item = &'labels str>,
    container_name: &str,
) -> bool {
    label_keys.any(|label_key| WOODPECKER_LABELS.contains(&label_key))
        || parse_forgejo_container_name(container_name).is_some()
}

pub(crate) struct PhasedContainers<Listed> {
    pub(crate) running: Vec<Listed>,
    pub(crate) stopped: Vec<StoppedContainer>,
}

pub(crate) fn phased_containers<Listed>(
    listed: Vec<Listed>,
    phase_of: impl Fn(&Listed) -> ContainerPhase,
    stopped_of: impl Fn(&Listed) -> StoppedContainer,
) -> PhasedContainers<Listed> {
    let mut phased = listed.into_iter().fold(
        PhasedContainers {
            running: Vec::new(),
            stopped: Vec::new(),
        },
        |mut phased, container| {
            match phase_of(&container) {
                ContainerPhase::Running => phased.running.push(container),
                ContainerPhase::Reusable => phased.stopped.push(stopped_of(&container)),
                ContainerPhase::CiLeftover | ContainerPhase::Transitional => {}
            }
            phased
        },
    );
    phased
        .stopped
        .sort_by_key(|stopped| Reverse(stopped.stopped_at));
    phased
}

impl EngineContainers {
    #[must_use]
    pub fn total_resident_memory(&self) -> ByteSize {
        self.containers
            .iter()
            .fold(ByteSize::b(0), |total, container| {
                total + container.resident_memory
            })
    }

    #[must_use]
    pub fn total_cpu_load(&self) -> ProcessTreeCpuLoad {
        total_cpu_load(self.containers.iter().map(|container| container.cpu_load))
    }
}

fn engine_unix_time(unix_seconds: i64) -> Option<SystemTime> {
    match u64::try_from(unix_seconds) {
        Ok(positive_seconds) if positive_seconds > 0 => {
            Some(UNIX_EPOCH + Duration::from_secs(positive_seconds))
        }
        Ok(_) | Err(_) => None,
    }
}

pub(crate) fn parsed_engine_timestamp(timestamp_text: &str) -> Option<SystemTime> {
    if timestamp_text.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(timestamp_text)
        .inspect_err(|parse_failure| {
            tracing::debug!(%timestamp_text, %parse_failure, "container engine timestamp is malformed");
        })
        .ok()
        .map(SystemTime::from)
}

pub(crate) async fn classified_engine_failure(
    socket_path: &Path,
    fetch_failure: http_json_client::Error,
) -> EngineReadError {
    match UnixStream::connect(socket_path)
        .await
        .map_err(|failure| failure.kind())
    {
        Err(ErrorKind::NotFound) => EngineReadError::SocketMissing,
        Err(ErrorKind::PermissionDenied) => EngineReadError::SocketDenied,
        Ok(_) | Err(_) => EngineReadError::Fetch(fetch_failure),
    }
}

impl EngineListing {
    pub(crate) fn sampling_need(&self) -> SamplingNeed {
        if self.running.containers.is_empty() {
            SamplingNeed::Settled
        } else {
            SamplingNeed::KeepSampling
        }
    }
}

impl SnapshotSource for EngineListing {
    type Error = EngineReadError;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Podman containers";

    fn delivery_stream(socket_path: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        deliveries_on_running_set_changes(
            EngineSampler::new(socket_path),
            |mut sampler| async move {
                let snapshot_fetch = sampler.sample_listing().await;
                CarriedRead {
                    carried_state: sampler,
                    snapshot_fetch,
                }
            },
            |delivered| {
                delivered
                    .as_ref()
                    .map_or(SamplingNeed::KeepSampling, EngineListing::sampling_need)
            },
        )
        .map(unavailable_when_socket_missing)
    }
}
