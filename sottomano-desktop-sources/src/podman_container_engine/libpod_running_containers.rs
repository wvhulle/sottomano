use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fmt::{Display, Formatter, Result as FmtResult},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt};
use serde::Deserialize;
use tokio::{net::UnixStream, sync::Mutex};

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    podman_container_engine::{
        container_process_usage::{ProcessTable, busy_cores},
        libpod_inspect_ci_identity::{WoodpeckerStepIdentity, fetch_woodpecker_step_identity},
        running_set_changes::{ChangeTrackedSample, samples_on_running_set_changes},
    },
    stream_operators::StreamOperators,
    system_protocol_clients::http_json_client,
};

pub(super) const LIBPOD_API_ROOT: &str = "http://podman/v5.0.0/libpod";
pub(crate) const ENGINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const WOODPECKER_STEP_LABEL: &str = "wp_step";
const RUNNING_STATE: &str = "running";

#[derive(Debug, Clone, PartialEq)]
pub enum EngineReadError {
    SocketMissing,
    SocketDenied,
    Fetch(http_json_client::Error),
    ProcessTableUnreadable,
    ReadPanicked,
}

impl Display for EngineReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            EngineReadError::SocketMissing => f.write_str("the engine socket does not exist"),
            EngineReadError::SocketDenied => {
                f.write_str("this user may not open the engine socket")
            }
            EngineReadError::Fetch(failure) => failure.fmt(f),
            EngineReadError::ProcessTableUnreadable => f.write_str("cannot read /proc"),
            EngineReadError::ReadPanicked => Panicked.fmt(f),
        }
    }
}

impl From<Panicked> for EngineReadError {
    fn from(Panicked: Panicked) -> Self {
        EngineReadError::ReadPanicked
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerCpuLoad {
    AwaitingSecondSample,
    BusyCores(f64),
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
    pub cpu_load: ContainerCpuLoad,
    pub resident_memory: ByteSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineContainers {
    pub containers: Vec<RunningContainer>,
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
    pub fn total_cpu_load(&self) -> ContainerCpuLoad {
        total_cpu_load(self.containers.iter().map(|container| container.cpu_load))
    }
}

pub fn total_cpu_load(loads: impl Iterator<Item = ContainerCpuLoad>) -> ContainerCpuLoad {
    loads.fold(
        ContainerCpuLoad::AwaitingSecondSample,
        |total, load| match (total, load) {
            (ContainerCpuLoad::BusyCores(first), ContainerCpuLoad::BusyCores(second)) => {
                ContainerCpuLoad::BusyCores(first + second)
            }
            (known @ ContainerCpuLoad::BusyCores(_), ContainerCpuLoad::AwaitingSecondSample) => {
                known
            }
            (ContainerCpuLoad::AwaitingSecondSample, other) => other,
        },
    )
}

#[derive(Deserialize)]
struct ListedContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "State", default)]
    state: String,
    #[serde(rename = "Pid", default)]
    pid: i32,
    #[serde(rename = "StartedAt", default)]
    started_at_unix_seconds: u64,
    #[serde(rename = "Labels", default)]
    labels: Option<HashMap<String, String>>,
}

impl ListedContainer {
    fn step_label(&self) -> Option<&str> {
        self.labels
            .as_ref()?
            .get(WOODPECKER_STEP_LABEL)
            .map(String::as_str)
    }

    fn container_name(&self) -> String {
        self.names
            .first()
            .cloned()
            .unwrap_or_else(|| self.id.clone())
    }

    fn is_measurable(&self) -> bool {
        self.state == RUNNING_STATE && self.pid > 0
    }
}

async fn checked_socket_access(socket_path: &Path) -> Result<(), EngineReadError> {
    UnixStream::connect(socket_path)
        .await
        .map(drop)
        .map_err(|failure| match failure.kind() {
            ErrorKind::NotFound => EngineReadError::SocketMissing,
            ErrorKind::PermissionDenied => EngineReadError::SocketDenied,
            _ => EngineReadError::Fetch(http_json_client::Error::Unreachable {
                host: socket_path.display().to_string(),
                detail: failure.to_string(),
            }),
        })
}

struct CpuCheckpoint {
    spent_ticks: u64,
    taken_at: Instant,
}

pub(crate) struct EngineSampler {
    socket_path: PathBuf,
    identities_by_id: HashMap<String, WoodpeckerStepIdentity>,
    checkpoints_by_id: HashMap<String, CpuCheckpoint>,
}

impl EngineSampler {
    pub(crate) fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            identities_by_id: HashMap::new(),
            checkpoints_by_id: HashMap::new(),
        }
    }

    async fn learn_new_identities(&mut self, listed: &[ListedContainer]) {
        let unknown_step_ids: Vec<String> = listed
            .iter()
            .filter(|container| container.step_label().is_some())
            .filter(|container| !self.identities_by_id.contains_key(&container.id))
            .map(|container| container.id.clone())
            .collect();
        for container_id in unknown_step_ids {
            if let Some(identity) =
                fetch_woodpecker_step_identity(&self.socket_path, &container_id).await
            {
                self.identities_by_id.insert(container_id, identity);
            }
        }
    }

    pub(crate) async fn sample(&mut self) -> Result<EngineContainers, EngineReadError> {
        checked_socket_access(&self.socket_path).await?;
        let listed: Vec<ListedContainer> = http_json_client::get_json_over_unix_socket(
            &self.socket_path,
            &format!("{LIBPOD_API_ROOT}/containers/json"),
        )
        .await
        .map_err(EngineReadError::Fetch)?;
        let listed: Vec<ListedContainer> = listed
            .into_iter()
            .filter(ListedContainer::is_measurable)
            .collect();
        if listed.is_empty() {
            self.identities_by_id.clear();
            self.checkpoints_by_id.clear();
            return Ok(EngineContainers {
                containers: Vec::new(),
            });
        }
        self.learn_new_identities(&listed).await;
        let table = blocking_poll(|| {
            ProcessTable::read_current().ok_or(EngineReadError::ProcessTableUnreadable)
        })
        .await?;
        Ok(self.measured(&listed, &table, Instant::now()))
    }

    fn measured(
        &mut self,
        listed: &[ListedContainer],
        table: &ProcessTable,
        taken_at: Instant,
    ) -> EngineContainers {
        let live_ids: HashSet<&str> = listed
            .iter()
            .map(|container| container.id.as_str())
            .collect();
        self.identities_by_id
            .retain(|container_id, _| live_ids.contains(container_id.as_str()));
        self.checkpoints_by_id
            .retain(|container_id, _| live_ids.contains(container_id.as_str()));
        let containers = listed
            .iter()
            .map(|container| self.measured_container(container, table, taken_at))
            .collect();
        EngineContainers { containers }
    }

    fn measured_container(
        &mut self,
        container: &ListedContainer,
        table: &ProcessTable,
        taken_at: Instant,
    ) -> RunningContainer {
        let totals = table.tree_totals(container.pid);
        let cpu_load = self.checkpoints_by_id.get(&container.id).map_or(
            ContainerCpuLoad::AwaitingSecondSample,
            |checkpoint| {
                ContainerCpuLoad::BusyCores(busy_cores(
                    totals.spent_ticks.saturating_sub(checkpoint.spent_ticks),
                    taken_at.duration_since(checkpoint.taken_at).as_secs_f64(),
                ))
            },
        );
        self.checkpoints_by_id.insert(
            container.id.clone(),
            CpuCheckpoint {
                spent_ticks: totals.spent_ticks,
                taken_at,
            },
        );
        let container_name = container.container_name();
        RunningContainer {
            id: container.id.clone(),
            shown_name: container
                .step_label()
                .map_or_else(|| container_name.clone(), str::to_owned),
            container_name,
            image: container.image.clone(),
            started_at: UNIX_EPOCH + Duration::from_secs(container.started_at_unix_seconds),
            ci_identity: self
                .identities_by_id
                .get(&container.id)
                .map_or(ContainerCiIdentity::NotWoodpecker, |identity| {
                    ContainerCiIdentity::WoodpeckerStep(identity.clone())
                }),
            cpu_load,
            resident_memory: ByteSize::b(totals.resident_bytes),
        }
    }
}

pub(crate) type EngineRead = Result<EngineContainers, EngineReadError>;

fn engine_keeps_sampling(engine_read: &EngineRead) -> bool {
    !matches!(engine_read, Ok(engine) if engine.containers.is_empty())
}

pub(crate) fn shared_engine_sampler(socket_path: PathBuf) -> Arc<Mutex<EngineSampler>> {
    Arc::new(Mutex::new(EngineSampler::new(socket_path)))
}

pub(crate) fn engine_samples_on_running_set_changes(
    socket_path: PathBuf,
) -> impl Stream<Item = ChangeTrackedSample<EngineRead>> + Send {
    let sampler = shared_engine_sampler(socket_path);
    samples_on_running_set_changes(
        move || {
            let sampler = sampler.clone();
            async move { sampler.lock().await.sample().await }
        },
        engine_keeps_sampling,
    )
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineReadout {
    Containers(EngineContainers),
    Unreadable(EngineReadError),
}

impl From<EngineRead> for EngineReadout {
    fn from(engine_read: EngineRead) -> Self {
        match engine_read {
            Ok(engine) => EngineReadout::Containers(engine),
            Err(failure) => EngineReadout::Unreadable(failure),
        }
    }
}

impl SnapshotSource for EngineReadout {
    type Error = Infallible;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Podman containers";

    fn delivery_stream(socket_path: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        engine_samples_on_running_set_changes(socket_path)
            .map(|tracked| tracked.stamped_cadence.delivered(Ok(tracked.sample.into())))
            .repeats_as_unchanged()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::podman_container_engine::container_process_usage::ProcessSample;

    fn listed_step_container() -> ListedContainer {
        serde_json::from_value(json!({
            "Id": "abc",
            "Names": ["wp_01M6"],
            "Image": "docker.io/nixos/nix:latest",
            "State": "running",
            "Pid": 10,
            "StartedAt": 1_793_649_940,
            "Labels": {"wp_step": "clone", "wp_uuid": "01M6"},
        }))
        .expect("listing decodes")
    }

    #[test]
    fn exited_and_pidless_containers_are_not_measured() {
        let exited: ListedContainer =
            serde_json::from_value(json!({"Id": "abc", "State": "exited", "Pid": 0}))
                .expect("listing decodes");
        assert!(!exited.is_measurable());
        assert!(listed_step_container().is_measurable());
    }

    #[test]
    fn a_container_is_shown_by_its_step_label() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let table = ProcessTable::from_samples([]);
        let measured = sampler.measured(&[listed_step_container()], &table, Instant::now());
        assert_eq!(measured.containers[0].shown_name, "clone");
        assert_eq!(measured.containers[0].container_name, "wp_01M6");
    }

    #[test]
    fn cpu_load_needs_two_samples_and_memory_is_the_tree_sum() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let started = Instant::now();
        let first_table = ProcessTable::from_samples([ProcessSample {
            pid: 10,
            parent_pid: 1,
            spent_ticks: 100,
            resident_bytes: 4096,
        }]);
        let first = sampler.measured(&[listed_step_container()], &first_table, started);
        assert_eq!(
            first.containers[0].cpu_load,
            ContainerCpuLoad::AwaitingSecondSample
        );
        assert_eq!(first.containers[0].resident_memory, ByteSize::b(4096));

        let second_table = ProcessTable::from_samples([ProcessSample {
            pid: 10,
            parent_pid: 1,
            spent_ticks: 100 + procfs::ticks_per_second() * 10,
            resident_bytes: 4096,
        }]);
        let second = sampler.measured(
            &[listed_step_container()],
            &second_table,
            started + Duration::from_secs(5),
        );
        let ContainerCpuLoad::BusyCores(cores) = second.containers[0].cpu_load else {
            panic!("a second sample yields a load");
        };
        assert!((cores - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_shrinking_tree_never_reports_negative_load() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let started = Instant::now();
        let sample_with_ticks = |spent_ticks| {
            ProcessTable::from_samples([ProcessSample {
                pid: 10,
                parent_pid: 1,
                spent_ticks,
                resident_bytes: 0,
            }])
        };
        sampler.measured(&[listed_step_container()], &sample_with_ticks(500), started);
        let shrunk = sampler.measured(
            &[listed_step_container()],
            &sample_with_ticks(100),
            started + Duration::from_secs(5),
        );
        assert_eq!(
            shrunk.containers[0].cpu_load,
            ContainerCpuLoad::BusyCores(0.0)
        );
    }

    #[test]
    fn only_a_readable_empty_engine_stops_usage_sampling() {
        let empty: EngineRead = Ok(EngineContainers {
            containers: Vec::new(),
        });
        assert!(!engine_keeps_sampling(&empty));
        assert!(engine_keeps_sampling(&Err(EngineReadError::SocketMissing)));
    }

    #[test]
    fn gone_containers_are_forgotten() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let table = ProcessTable::from_samples([]);
        sampler.measured(&[listed_step_container()], &table, Instant::now());
        assert_eq!(sampler.checkpoints_by_id.len(), 1);
        sampler.measured(&[], &table, Instant::now());
        assert!(sampler.checkpoints_by_id.is_empty());
    }

    #[test]
    fn totals_ignore_containers_still_awaiting_a_sample() {
        assert_eq!(
            total_cpu_load(
                [
                    ContainerCpuLoad::BusyCores(1.5),
                    ContainerCpuLoad::AwaitingSecondSample,
                    ContainerCpuLoad::BusyCores(0.5),
                ]
                .into_iter()
            ),
            ContainerCpuLoad::BusyCores(2.0)
        );
        assert_eq!(
            total_cpu_load([ContainerCpuLoad::AwaitingSecondSample].into_iter()),
            ContainerCpuLoad::AwaitingSecondSample
        );
    }

    #[tokio::test]
    async fn a_missing_socket_is_distinct_from_a_denied_one() {
        assert_eq!(
            checked_socket_access(Path::new("/nonexistent/podman.sock")).await,
            Err(EngineReadError::SocketMissing)
        );
    }
}
