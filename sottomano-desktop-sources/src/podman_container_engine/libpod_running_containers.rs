use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bytesize::ByteSize;
use futures::Stream;
use serde::Deserialize;
use tokio::{net::UnixStream, sync::Mutex};

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    delivery_cadence_drivers::SamplingNeed,
    podman_container_engine::{
        libpod_inspect_ci_identity::{WoodpeckerStepIdentity, fetch_woodpecker_step_identity},
        procfs_conmon_sessions::ConmonSessions,
        running_set_changes::deliveries_on_running_set_changes,
    },
    system_protocol_clients::{
        http_json_client,
        journal_follow::JournalFollowError,
        procfs_process_tree::{
            CpuTickCheckpoints, ProcessTable, ProcessTreeCpuLoad, ProcessTreeTotals, total_cpu_load,
        },
    },
};

pub const LIBPOD_API_ROOT: &str = "http://podman/v5.0.0/libpod";
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

impl From<http_json_client::Error> for EngineReadError {
    fn from(fetch_failure: http_json_client::Error) -> Self {
        EngineReadError::Fetch(fetch_failure)
    }
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

    fn process_state(&self) -> ContainerProcessState {
        if self.state == RUNNING_STATE && self.pid > 0 {
            ContainerProcessState::RunningProcess
        } else {
            ContainerProcessState::NoProcess
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerProcessState {
    RunningProcess,
    NoProcess,
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

async fn listed_containers(
    socket_path: &Path,
) -> Result<Vec<ListedContainer>, http_json_client::Error> {
    http_json_client::get_json_over_unix_socket(
        socket_path,
        &format!("{LIBPOD_API_ROOT}/containers/json"),
    )
    .await
}

async fn listed_containers_with_one_retry(
    socket_path: &Path,
) -> Result<Vec<ListedContainer>, EngineReadError> {
    let attempt = match listed_containers(socket_path).await {
        Err(http_json_client::Error::Status { status, .. })
            if status == http_json_client::TOO_MANY_REQUESTS_STATUS
                || status >= http_json_client::SERVER_ERROR_STATUS =>
        {
            listed_containers(socket_path).await
        }
        Err(
            http_json_client::Error::HostNotResolved { .. }
            | http_json_client::Error::TimedOut { .. }
            | http_json_client::Error::Unreachable { .. },
        ) => listed_containers(socket_path).await,
        first_attempt => first_attempt,
    };
    match attempt {
        Ok(listed) => Ok(listed),
        Err(failure) => Err(classified_engine_failure(socket_path, failure).await),
    }
}

pub(crate) struct HostProcesses {
    process_table: ProcessTable,
    conmon_sessions: ConmonSessions,
}

impl HostProcesses {
    fn read_current() -> Option<Self> {
        let process_table = ProcessTable::read_current()?;
        let conmon_sessions = ConmonSessions::read_current(&process_table);
        Some(Self {
            process_table,
            conmon_sessions,
        })
    }

    fn container_totals(&self, container: &ListedContainer) -> ProcessTreeTotals {
        self.process_table.forest_totals(
            self.conmon_sessions
                .measured_roots(&container.id, container.pid),
        )
    }
}

pub(crate) struct EngineSampler {
    socket_path: PathBuf,
    identities_by_id: HashMap<String, WoodpeckerStepIdentity>,
    checkpoints_by_id: CpuTickCheckpoints<String>,
}

impl EngineSampler {
    pub(crate) fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            identities_by_id: HashMap::new(),
            checkpoints_by_id: CpuTickCheckpoints::default(),
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
        let listed: Vec<ListedContainer> = listed_containers_with_one_retry(&self.socket_path)
            .await?
            .into_iter()
            .filter(|listed| listed.process_state() == ContainerProcessState::RunningProcess)
            .collect();
        if listed.is_empty() {
            self.identities_by_id.clear();
            self.checkpoints_by_id.clear();
            return Ok(EngineContainers {
                containers: Vec::new(),
            });
        }
        self.learn_new_identities(&listed).await;
        let host_processes = blocking_poll(|| {
            HostProcesses::read_current().ok_or(EngineReadError::ProcessTableUnreadable)
        })
        .await?;
        Ok(self.measured(&listed, &host_processes, Instant::now()))
    }

    fn measured(
        &mut self,
        listed: &[ListedContainer],
        host_processes: &HostProcesses,
        taken_at: Instant,
    ) -> EngineContainers {
        let live_ids: HashSet<&str> = listed
            .iter()
            .map(|container| container.id.as_str())
            .collect();
        self.identities_by_id
            .retain(|container_id, _| live_ids.contains(container_id.as_str()));
        self.checkpoints_by_id
            .retain_keys(|container_id| live_ids.contains(container_id.as_str()));
        let containers = listed
            .iter()
            .map(|container| self.measured_container(container, host_processes, taken_at))
            .collect();
        EngineContainers { containers }
    }

    fn measured_container(
        &mut self,
        container: &ListedContainer,
        host_processes: &HostProcesses,
        taken_at: Instant,
    ) -> RunningContainer {
        let totals = host_processes.container_totals(container);
        let cpu_load = self.checkpoints_by_id.load_since_previous(
            container.id.clone(),
            totals.spent_ticks,
            taken_at,
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

pub(crate) fn shared_engine_sampler(socket_path: PathBuf) -> Arc<Mutex<EngineSampler>> {
    Arc::new(Mutex::new(EngineSampler::new(socket_path)))
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineReadout {
    Containers(EngineContainers),
    Unreadable(EngineReadError),
}

impl From<Result<EngineContainers, EngineReadError>> for EngineReadout {
    fn from(engine_read: Result<EngineContainers, EngineReadError>) -> Self {
        match engine_read {
            Ok(engine) => EngineReadout::Containers(engine),
            Err(failure) => EngineReadout::Unreadable(failure),
        }
    }
}

impl EngineReadout {
    pub(crate) fn sampling_need(&self) -> SamplingNeed {
        match self {
            EngineReadout::Containers(engine) if engine.containers.is_empty() => {
                SamplingNeed::Settled
            }
            EngineReadout::Containers(_) | EngineReadout::Unreadable(_) => {
                SamplingNeed::KeepSampling
            }
        }
    }
}

impl SnapshotSource for EngineReadout {
    type Error = JournalFollowError;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Podman containers";

    fn delivery_stream(socket_path: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let sampler = shared_engine_sampler(socket_path);
        deliveries_on_running_set_changes(
            move || {
                let sampler = sampler.clone();
                async move { Ok(sampler.lock().await.sample().await.into()) }
            },
            |delivered| {
                delivered
                    .as_ref()
                    .map_or(SamplingNeed::KeepSampling, EngineReadout::sampling_need)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixListener,
    };
    use uom::si::{f64::Ratio, ratio::percent};

    use super::*;
    use crate::{
        desktop_windows_apps::process_tree_inspection::CpuCapacity,
        system_protocol_clients::procfs_process_tree::ProcessSample,
    };

    fn processes_without_conmon(samples: impl IntoIterator<Item = ProcessSample>) -> HostProcesses {
        HostProcesses {
            process_table: ProcessTable::from_samples(samples),
            conmon_sessions: ConmonSessions::default(),
        }
    }

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
        assert_eq!(exited.process_state(), ContainerProcessState::NoProcess);
        assert_eq!(
            listed_step_container().process_state(),
            ContainerProcessState::RunningProcess
        );
    }

    #[test]
    fn a_container_is_shown_by_its_step_label() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let table = processes_without_conmon([]);
        let measured = sampler.measured(&[listed_step_container()], &table, Instant::now());
        assert_eq!(measured.containers[0].shown_name, "clone");
        assert_eq!(measured.containers[0].container_name, "wp_01M6");
    }

    #[test]
    fn cpu_load_needs_two_samples_and_memory_is_the_tree_sum() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        sampler.checkpoints_by_id = CpuTickCheckpoints::with_capacity(CpuCapacity::from(4));
        let started = Instant::now();
        let first_table = processes_without_conmon([ProcessSample {
            pid: 10,
            parent_pid: 1,
            spent_ticks: 100,
            resident_bytes: 4096,
        }]);
        let first = sampler.measured(&[listed_step_container()], &first_table, started);
        assert_eq!(
            first.containers[0].cpu_load,
            ProcessTreeCpuLoad::AwaitingSecondSample
        );
        assert_eq!(first.containers[0].resident_memory, ByteSize::b(4096));

        let second_table = processes_without_conmon([ProcessSample {
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
        let ProcessTreeCpuLoad::BusyShare(share) = second.containers[0].cpu_load else {
            panic!("a second sample yields a load");
        };
        assert!((share.get::<percent>() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn exec_sessions_count_toward_their_container_only() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let process = |pid, parent_pid, resident_bytes| ProcessSample {
            pid,
            parent_pid,
            spent_ticks: 0,
            resident_bytes,
        };
        let host_processes = HostProcesses {
            process_table: ProcessTable::from_samples([
                process(9, 1, 100),
                process(10, 9, 140),
                process(20, 1, 100),
                process(21, 20, 1_000_000),
                process(30, 1, 100),
                process(31, 30, 5_000_000),
            ]),
            conmon_sessions: ConmonSessions::from_command_lines([
                (
                    9,
                    vec!["conmon".to_owned(), "-c".to_owned(), "abc".to_owned()],
                ),
                (
                    20,
                    vec![
                        "conmon".to_owned(),
                        "-c".to_owned(),
                        "abc".to_owned(),
                        "--exec".to_owned(),
                    ],
                ),
                (
                    30,
                    vec!["conmon".to_owned(), "-c".to_owned(), "other".to_owned()],
                ),
            ]),
        };
        let measured =
            sampler.measured(&[listed_step_container()], &host_processes, Instant::now());
        assert_eq!(
            measured.containers[0].resident_memory,
            ByteSize::b(100 + 140 + 100 + 1_000_000)
        );
    }

    #[test]
    fn a_shrinking_tree_never_reports_negative_load() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let started = Instant::now();
        let sample_with_ticks = |spent_ticks| {
            processes_without_conmon([ProcessSample {
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
            ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(0.0))
        );
    }

    #[test]
    fn only_a_readable_empty_engine_stops_usage_sampling() {
        let empty = EngineReadout::Containers(EngineContainers {
            containers: Vec::new(),
        });
        assert_eq!(empty.sampling_need(), SamplingNeed::Settled);
        assert_eq!(
            EngineReadout::Unreadable(EngineReadError::SocketMissing).sampling_need(),
            SamplingNeed::KeepSampling
        );
    }

    #[test]
    fn gone_containers_are_forgotten() {
        let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
        let table = processes_without_conmon([]);
        sampler.measured(&[listed_step_container()], &table, Instant::now());
        assert_eq!(sampler.checkpoints_by_id.len(), 1);
        sampler.measured(&[], &table, Instant::now());
        assert!(sampler.checkpoints_by_id.is_empty());
    }

    #[tokio::test]
    async fn a_missing_socket_is_distinct_from_a_denied_one() {
        assert_eq!(
            listed_containers_with_one_retry(Path::new("/nonexistent/podman.sock"))
                .await
                .err(),
            Some(EngineReadError::SocketMissing)
        );
    }

    #[tokio::test]
    async fn a_dropped_connection_is_retried_once() {
        let socket_directory =
            env::temp_dir().join(format!("sottomano-podman-retry-{}", process::id()));
        fs::create_dir_all(&socket_directory).expect("scratch directory");
        let socket_path = socket_directory.join("podman.sock");
        let _ = fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).expect("socket binds");
        let serving = tokio::spawn(async move {
            let (dropped_connection, _) = listener.accept().await.expect("first connection");
            drop(dropped_connection);
            let (mut answered_connection, _) = listener.accept().await.expect("second connection");
            let mut request_bytes = [0u8; 1024];
            let _ = answered_connection.read(&mut request_bytes).await;
            answered_connection
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]",
                )
                .await
                .expect("response writes");
        });
        let listed = listed_containers_with_one_retry(&socket_path)
            .await
            .expect("the retry is answered");
        assert!(listed.is_empty());
        serving.await.expect("server finishes");
        fs::remove_dir_all(&socket_directory).expect("scratch directory removed");
    }
}
