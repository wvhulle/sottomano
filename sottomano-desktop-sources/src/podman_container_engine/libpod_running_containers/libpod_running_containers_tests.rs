use std::{env, fs, process, time::Instant};

use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};
use uom::si::{f64::Ratio, ratio::percent};

use super::{
    libpod_engine_sampler::HostProcesses,
    libpod_listed_containers::{ListedContainer, listed_containers_with_one_retry},
    *,
};
use crate::{
    desktop_windows_apps::process_tree_inspection::CpuCapacity,
    podman_container_engine::procfs_conmon_sessions::ConmonSessions,
    system_protocol_clients::procfs_process_tree::{
        CpuTickCheckpoints, ProcessSample, ProcessTable,
    },
};

fn processes_without_conmon(samples: impl IntoIterator<Item = ProcessSample>) -> HostProcesses {
    HostProcesses {
        process_table: ProcessTable::from_iter(samples),
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
    let listed_phases: Vec<ContainerPhase> = listed.iter().map(ListedContainer::phase).collect();
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
        process_table: ProcessTable::from_iter([
            process(9, 1, 100),
            process(10, 9, 140),
            process(20, 1, 100),
            process(21, 20, 1_000_000),
            process(30, 1, 100),
            process(31, 30, 5_000_000),
        ]),
        conmon_sessions: ConmonSessions::from_iter([
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
    let measured = sampler.measured(&[listed_step_container()], &host_processes, Instant::now());
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
fn an_empty_engine_stops_usage_sampling() {
    let empty = EngineListing {
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
    };
    assert_eq!(empty.sampling_need(), SamplingNeed::Settled);
}

#[test]
fn gone_containers_are_forgotten() {
    let mut sampler = EngineSampler::new(PathBuf::from("/nonexistent"));
    let table = processes_without_conmon([]);
    sampler.measured(&[listed_step_container()], &table, Instant::now());
    assert_eq!(sampler.checkpoints_by_id.checkpoints_by_key.len(), 1);
    sampler.measured(&[], &table, Instant::now());
    assert!(sampler.checkpoints_by_id.checkpoints_by_key.is_empty());
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
