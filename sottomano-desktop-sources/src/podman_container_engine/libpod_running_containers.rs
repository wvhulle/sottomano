use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bytesize::ByteSize;
use chrono::DateTime;
use futures::Stream;
use serde::Deserialize;
use tokio::{net::UnixStream, sync::Mutex};

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    ci_runner_tasks::forgejo_runner_container_names::parse_forgejo_container_name,
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
        sysfs_cgroup_memory::unit_working_set_of_process,
    },
};

pub const LIBPOD_API_ROOT: &str = "http://podman/v5.0.0/libpod";
pub(crate) const ENGINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const WOODPECKER_STEP_LABEL: &str = "wp_step";
const WOODPECKER_LABELS: [&str; 2] = ["wp_uuid", WOODPECKER_STEP_LABEL];
const RUNNING_STATE: &str = "running";
const STOPPED_STATES: [&str; 4] = ["created", "configured", "exited", "stopped"];

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

pub(crate) fn created_by_ci_runner<'a>(
    mut label_keys: impl Iterator<Item = &'a str>,
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
    started_at_unix_seconds: i64,
    #[serde(rename = "ExitedAt", default)]
    exited_at_unix_seconds: i64,
    #[serde(rename = "Created", default)]
    created: String,
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

    fn phase(&self) -> ContainerPhase {
        if self.state == RUNNING_STATE && self.pid > 0 {
            ContainerPhase::Running
        } else if STOPPED_STATES.contains(&self.state.as_str()) {
            if created_by_ci_runner(
                self.labels
                    .iter()
                    .flatten()
                    .map(|(label_key, _)| label_key.as_str()),
                &self.container_name(),
            ) {
                ContainerPhase::CiLeftover
            } else {
                ContainerPhase::Reusable
            }
        } else {
            ContainerPhase::Transitional
        }
    }

    fn shown_name(&self) -> String {
        self.step_label()
            .map_or_else(|| self.container_name(), str::to_owned)
    }

    fn started_at(&self) -> SystemTime {
        engine_unix_time(self.started_at_unix_seconds).unwrap_or(UNIX_EPOCH)
    }

    fn stopped_container(&self) -> StoppedContainer {
        let stopped_at = engine_unix_time(self.exited_at_unix_seconds)
            .into_iter()
            .chain(parsed_engine_timestamp(&self.created))
            .max()
            .unwrap_or(UNIX_EPOCH);
        StoppedContainer {
            id: self.id.clone(),
            container_name: self.container_name(),
            shown_name: self.shown_name(),
            image: self.image.clone(),
            stopped_at,
        }
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

async fn listed_containers(
    socket_path: &Path,
    selection: ContainerSelection,
) -> Result<Vec<ListedContainer>, http_json_client::Error> {
    http_json_client::get_json_over_unix_socket(
        socket_path,
        &format!(
            "{LIBPOD_API_ROOT}/containers/json{}",
            selection.list_query()
        ),
    )
    .await
}

async fn listed_containers_with_one_retry(
    socket_path: &Path,
    selection: ContainerSelection,
) -> Result<Vec<ListedContainer>, EngineReadError> {
    let attempt = match listed_containers(socket_path, selection).await {
        Err(http_json_client::Error::Status { status, .. })
            if status == http_json_client::TOO_MANY_REQUESTS_STATUS
                || status >= http_json_client::SERVER_ERROR_STATUS =>
        {
            listed_containers(socket_path, selection).await
        }
        Err(
            http_json_client::Error::HostNotResolved { .. }
            | http_json_client::Error::TimedOut { .. }
            | http_json_client::Error::Unreachable { .. },
        ) => listed_containers(socket_path, selection).await,
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
    cgroup_memory_of: fn(u32) -> Option<ByteSize>,
}

impl HostProcesses {
    fn read_current() -> Option<Self> {
        let process_table = ProcessTable::read_current()?;
        let conmon_sessions = ConmonSessions::read_current(&process_table);
        Some(Self {
            process_table,
            conmon_sessions,
            cgroup_memory_of: unit_working_set_of_process,
        })
    }

    fn container_memory(&self, container: &ListedContainer, totals: ProcessTreeTotals) -> ByteSize {
        u32::try_from(container.pid)
            .inspect_err(|conversion_failure| {
                tracing::debug!(pid = container.pid, %conversion_failure, "container pid is not a process id");
            })
            .ok()
            .and_then(self.cgroup_memory_of)
            .unwrap_or(ByteSize::b(totals.resident_bytes))
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
        let listed: Vec<ListedContainer> =
            listed_containers_with_one_retry(&self.socket_path, ContainerSelection::Running)
                .await?
                .into_iter()
                .filter(|listed| listed.phase() == ContainerPhase::Running)
                .collect();
        self.measured_running(listed).await
    }

    async fn sample_listing(&mut self) -> Result<EngineListing, EngineReadError> {
        let phased = phased_containers(
            listed_containers_with_one_retry(&self.socket_path, ContainerSelection::All).await?,
            ListedContainer::phase,
            ListedContainer::stopped_container,
        );
        Ok(EngineListing {
            running: self.measured_running(phased.running).await?,
            stopped: phased.stopped,
        })
    }

    async fn measured_running(
        &mut self,
        listed: Vec<ListedContainer>,
    ) -> Result<EngineContainers, EngineReadError> {
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
        RunningContainer {
            id: container.id.clone(),
            shown_name: container.shown_name(),
            container_name: container.container_name(),
            image: container.image.clone(),
            started_at: container.started_at(),
            ci_identity: self
                .identities_by_id
                .get(&container.id)
                .map_or(ContainerCiIdentity::NotWoodpecker, |identity| {
                    ContainerCiIdentity::WoodpeckerStep(identity.clone())
                }),
            cpu_load,
            resident_memory: host_processes.container_memory(container, totals),
        }
    }
}

pub(crate) fn shared_engine_sampler(socket_path: PathBuf) -> Arc<Mutex<EngineSampler>> {
    Arc::new(Mutex::new(EngineSampler::new(socket_path)))
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineReadout {
    Containers(EngineListing),
    Unreadable(EngineReadError),
}

impl From<Result<EngineListing, EngineReadError>> for EngineReadout {
    fn from(engine_read: Result<EngineListing, EngineReadError>) -> Self {
        match engine_read {
            Ok(engine) => EngineReadout::Containers(engine),
            Err(failure) => EngineReadout::Unreadable(failure),
        }
    }
}

impl EngineReadout {
    pub(crate) fn sampling_need(&self) -> SamplingNeed {
        match self {
            EngineReadout::Containers(engine) if engine.running.containers.is_empty() => {
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
                async move { Ok(sampler.lock().await.sample_listing().await.into()) }
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
            cgroup_memory_of: |_| None,
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
    fn a_libpod_listing_with_text_creation_times_decodes() {
        let listed: Vec<ListedContainer> = serde_json::from_value(json!([
            {
                "Id": "exited",
                "State": "exited",
                "Pid": 0,
                "Created": "2026-11-05T20:52:02.775637078+01:00",
                "ExitedAt": 1_793_908_371,
                "StartedAt": 1_793_908_323
            },
            {
                "Id": "never-run",
                "State": "created",
                "Pid": 0,
                "Created": "2026-11-05T20:53:38.531269808+01:00",
                "ExitedAt": -62_135_596_800_i64,
                "StartedAt": -62_135_596_800_i64
            },
            {
                "Id": "running",
                "State": "running",
                "Pid": 4242,
                "Created": "2026-11-05T20:53:38.531269818+01:00",
                "ExitedAt": 0,
                "StartedAt": 1_793_908_420
            }
        ]))
        .expect("a real libpod listing decodes");
        let phases: Vec<ContainerPhase> = listed.iter().map(ListedContainer::phase).collect();
        assert_eq!(
            phases,
            [
                ContainerPhase::Reusable,
                ContainerPhase::Reusable,
                ContainerPhase::Running
            ]
        );
        assert_eq!(
            listed[0].stopped_container().stopped_at,
            UNIX_EPOCH + Duration::from_secs(1_793_908_371)
        );
        assert_eq!(
            listed[1].stopped_container().stopped_at,
            parsed_engine_timestamp("2026-11-05T20:53:38.531269808+01:00").expect("time parses")
        );
    }

    #[test]
    fn stopped_ci_steps_are_leftovers_and_other_stopped_containers_are_reusable() {
        let listed: Vec<ListedContainer> = serde_json::from_value(json!([
            {
                "Id": "woodpecker-step",
                "Names": ["wp_01M6PKMTW4MKS1F72D614QEZZF"],
                "State": "exited",
                "Pid": 0,
                "Labels": {"wp_step": "clone", "wp_uuid": "01M6PKMTW4MKS1F72D614QEZZF"}
            },
            {
                "Id": "forgejo-job",
                "Names": ["FORGEJO-ACTIONS-TASK-11624854_WORKFLOW-67d7596b_JOB-vm-tests"],
                "State": "exited",
                "Pid": 0,
                "Labels": {"org.opencontainers.image.title": "nix"}
            },
            {
                "Id": "database",
                "Names": ["postgres"],
                "State": "exited",
                "Pid": 0
            },
            {
                "Id": "running-step",
                "Names": ["wp_01M7B11VPDF6YZMANFK7VF5SGW"],
                "State": "running",
                "Pid": 4242,
                "Labels": {"wp_step": "test", "wp_uuid": "01M7B11VPDF6YZMANFK7VF5SGW"}
            }
        ]))
        .expect("listing decodes");
        let listed_phases: Vec<ContainerPhase> =
            listed.iter().map(ListedContainer::phase).collect();
        assert_eq!(
            listed_phases,
            [
                ContainerPhase::CiLeftover,
                ContainerPhase::CiLeftover,
                ContainerPhase::Reusable,
                ContainerPhase::Running
            ]
        );
        let phased = phased_containers(
            listed,
            ListedContainer::phase,
            ListedContainer::stopped_container,
        );
        assert_eq!(phased.running.len(), 1);
        let stopped_ids: Vec<&str> = phased
            .stopped
            .iter()
            .map(|stopped| stopped.id.as_str())
            .collect();
        assert_eq!(stopped_ids, ["database"]);
    }

    #[test]
    fn exited_and_pidless_containers_are_not_measured() {
        let exited: ListedContainer =
            serde_json::from_value(json!({"Id": "abc", "State": "exited", "Pid": 0}))
                .expect("listing decodes");
        assert_eq!(exited.phase(), ContainerPhase::Reusable);
        assert_eq!(listed_step_container().phase(), ContainerPhase::Running);
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
            cgroup_memory_of: |_| None,
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
        let empty = EngineReadout::Containers(EngineListing {
            running: EngineContainers {
                containers: Vec::new(),
            },
            stopped: vec![StoppedContainer {
                id: "def".to_owned(),
                container_name: "exited".to_owned(),
                shown_name: "exited".to_owned(),
                image: "alpine".to_owned(),
                stopped_at: UNIX_EPOCH,
            }],
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
            listed_containers_with_one_retry(
                Path::new("/nonexistent/podman.sock"),
                ContainerSelection::Running,
            )
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
        let listed = listed_containers_with_one_retry(&socket_path, ContainerSelection::All)
            .await
            .expect("the retry is answered");
        assert!(listed.is_empty());
        serving.await.expect("server finishes");
        fs::remove_dir_all(&socket_directory).expect("scratch directory removed");
    }
}
