use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use bytesize::ByteSize;
use futures::{Stream, future::join_all};
use rustix::process::getuid;
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking,
    delivery_cadence_drivers::{
        ChangeWatchFailure, PeriodicPolling, SamplingNeed, SnapshotDeliveryStreams,
        triggered_deliveries_with_recheck,
    },
    qemu_virtual_machines::{
        libvirt_domain_control::{LIBVIRT_DOMAIN_CONTROL_BUILT, libvirt_run_states},
        machined_libvirt_machines::libvirt_leader_pids,
        procfs_launch_context::launch_context_of,
        procfs_qemu_processes::{
            ProcessWorkingDirectory, QemuProcess, SubordinateUidRange, account_names_by_uid,
            scanned_qemu_processes, subordinate_uid_ranges,
        },
        qemu_command_line_arguments::{QemuLaunchArguments, QmpSocketAccess},
        qmp_monitor_events::{GuestRunState, query_qmp_run_state},
    },
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses,
        procfs_process_tree::{CpuTickCheckpoints, ProcessTreeCpuLoad},
    },
    systemd_units_journal::machined_machine_registry::machine_set_changes,
};

pub const VIRTUAL_MACHINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const TRIGGER_BURST_GAP: Duration = Duration::from_millis(200);
const NIX_BUILD_ACCOUNT_PREFIX: &str = "nixbld";
const AUTO_ALLOCATED_BUILD_UIDS: Range<u32> = 872_415_232..880_803_840;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualMachineOrigin {
    SessionUser,
    NixosTestBuild,
    LibvirtDomain,
    OtherAccount(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestPowerControl {
    QmpSocket(PathBuf),
    LibvirtDomain(String),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunningVirtualMachine {
    pub pid: i32,
    pub origin: VirtualMachineOrigin,
    pub launch_context: Option<String>,
    pub launch: QemuLaunchArguments,
    pub run_state: GuestRunState,
    pub power_control: GuestPowerControl,
    pub started_at: SystemTime,
    pub cpu_load: ProcessTreeCpuLoad,
    pub resident_memory: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct VirtualMachinesReadout {
    pub machines: Vec<RunningVirtualMachine>,
}

fn origin_of(
    qemu_process: &QemuProcess,
    libvirt_pids: &HashSet<i32>,
    session_uid: u32,
    known_accounts: &KnownAccounts,
) -> VirtualMachineOrigin {
    if libvirt_pids.contains(&qemu_process.pid) {
        return VirtualMachineOrigin::LibvirtDomain;
    }
    if qemu_process.owner_uid == session_uid {
        return VirtualMachineOrigin::SessionUser;
    }
    let owner_uid = qemu_process.owner_uid;
    match known_accounts.names_by_uid.get(&owner_uid) {
        Some(account_name) if account_name.starts_with(NIX_BUILD_ACCOUNT_PREFIX) => {
            VirtualMachineOrigin::NixosTestBuild
        }
        Some(account_name) => VirtualMachineOrigin::OtherAccount(account_name.clone()),
        None if AUTO_ALLOCATED_BUILD_UIDS.contains(&owner_uid) => {
            VirtualMachineOrigin::NixosTestBuild
        }
        None => VirtualMachineOrigin::OtherAccount(
            known_accounts
                .subordinate_ranges
                .iter()
                .find(|subordinate_range| subordinate_range.contains(owner_uid))
                .map_or_else(
                    || owner_uid.to_string(),
                    |subordinate_range| format!("a container of {}", subordinate_range.owner_name),
                ),
        ),
    }
}

#[derive(Default)]
struct KnownAccounts {
    names_by_uid: HashMap<u32, String>,
    subordinate_ranges: Vec<SubordinateUidRange>,
}

fn read_known_accounts() -> KnownAccounts {
    KnownAccounts {
        names_by_uid: account_names_by_uid(),
        subordinate_ranges: subordinate_uid_ranges(),
    }
}

fn launch_arguments_of(qemu_process: &QemuProcess) -> QemuLaunchArguments {
    let launch = QemuLaunchArguments::parsed(&qemu_process.command_line);
    match &qemu_process.working_directory {
        ProcessWorkingDirectory::Readable(working_directory) => QemuLaunchArguments {
            qmp_socket: launch.qmp_socket.resolved_in(working_directory),
            ..launch
        },
        ProcessWorkingDirectory::Unreadable => launch,
    }
}

struct IdentifiedQemuProcess {
    qemu_process: QemuProcess,
    launch: QemuLaunchArguments,
    origin: VirtualMachineOrigin,
}

enum QmpStatus {
    Connected {
        socket_path: PathBuf,
        run_state: GuestRunState,
    },
    Unreachable,
}

async fn probe_qmp_status(qmp_socket: &QmpSocketAccess) -> QmpStatus {
    let Some(socket_path) = qmp_socket.connectable_path() else {
        return QmpStatus::Unreachable;
    };
    match query_qmp_run_state(socket_path).await {
        Ok(run_state) => QmpStatus::Connected {
            socket_path: socket_path.to_path_buf(),
            run_state,
        },
        Err(failure) => {
            tracing::debug!("qmp {}: {failure}", socket_path.display());
            QmpStatus::Unreachable
        }
    }
}

fn power_control_of(
    identified: &IdentifiedQemuProcess,
    qmp_status: &QmpStatus,
) -> GuestPowerControl {
    if identified.origin == VirtualMachineOrigin::LibvirtDomain && LIBVIRT_DOMAIN_CONTROL_BUILT {
        return GuestPowerControl::LibvirtDomain(identified.launch.machine_name.clone());
    }
    match qmp_status {
        QmpStatus::Connected { socket_path, .. } => {
            GuestPowerControl::QmpSocket(socket_path.clone())
        }
        QmpStatus::Unreachable => GuestPowerControl::Unavailable,
    }
}

pub(crate) struct VirtualMachineSampler {
    system_bus: zbus::Connection,
    session_uid: u32,
    checkpoints_by_pid: CpuTickCheckpoints<i32>,
}

impl VirtualMachineSampler {
    fn new(system_bus: zbus::Connection) -> Self {
        Self {
            system_bus,
            session_uid: getuid().as_raw(),
            checkpoints_by_pid: CpuTickCheckpoints::default(),
        }
    }

    async fn sample(&mut self) -> VirtualMachinesReadout {
        let scan = blocking("qemu process scan", scanned_qemu_processes).await;
        if scan.qemu_processes.is_empty() {
            self.checkpoints_by_pid.clear();
            return VirtualMachinesReadout::default();
        }
        let libvirt_pids = libvirt_leader_pids(&self.system_bus).await;
        let session_uid = self.session_uid;
        let known_accounts = if scan
            .qemu_processes
            .iter()
            .any(|qemu_process| qemu_process.owner_uid != session_uid)
        {
            blocking("account names", read_known_accounts).await
        } else {
            KnownAccounts::default()
        };
        let identified_processes: Vec<IdentifiedQemuProcess> = scan
            .qemu_processes
            .into_iter()
            .map(|qemu_process| IdentifiedQemuProcess {
                launch: launch_arguments_of(&qemu_process),
                origin: origin_of(&qemu_process, &libvirt_pids, session_uid, &known_accounts),
                qemu_process,
            })
            .collect();
        let libvirt_domain_names: Vec<String> = identified_processes
            .iter()
            .filter(|identified| identified.origin == VirtualMachineOrigin::LibvirtDomain)
            .map(|identified| identified.launch.machine_name.clone())
            .collect();
        let libvirt_states = if libvirt_domain_names.is_empty() {
            HashMap::new()
        } else {
            blocking("libvirt domain states", move || {
                libvirt_run_states(&libvirt_domain_names)
            })
            .await
        };
        let live_pids: HashSet<i32> = identified_processes
            .iter()
            .map(|identified| identified.qemu_process.pid)
            .collect();
        self.checkpoints_by_pid
            .retain_keys(|pid| live_pids.contains(pid));
        let qmp_statuses = join_all(
            identified_processes
                .iter()
                .map(|identified| probe_qmp_status(&identified.launch.qmp_socket)),
        )
        .await;
        let taken_at = Instant::now();
        let mut machines: Vec<RunningVirtualMachine> = identified_processes
            .into_iter()
            .zip(qmp_statuses)
            .map(|(identified, qmp_status)| {
                let pid = identified.qemu_process.pid;
                let totals = scan.process_table.tree_totals(pid);
                let qmp_run_state = match &qmp_status {
                    QmpStatus::Connected { run_state, .. } => Some(*run_state),
                    QmpStatus::Unreachable => None,
                };
                let run_state = libvirt_states
                    .get(&identified.launch.machine_name)
                    .copied()
                    .filter(|_| identified.origin == VirtualMachineOrigin::LibvirtDomain)
                    .or(qmp_run_state)
                    .unwrap_or(GuestRunState::Unreported);
                RunningVirtualMachine {
                    pid,
                    launch_context: (identified.origin != VirtualMachineOrigin::LibvirtDomain)
                        .then(|| launch_context_of(pid))
                        .flatten(),
                    power_control: power_control_of(&identified, &qmp_status),
                    cpu_load: self.checkpoints_by_pid.load_since_previous(
                        pid,
                        totals.spent_ticks,
                        taken_at,
                    ),
                    resident_memory: ByteSize::b(totals.resident_bytes),
                    started_at: identified.qemu_process.started_at,
                    run_state,
                    origin: identified.origin,
                    launch: identified.launch,
                }
            })
            .collect();
        machines.sort_by_key(|machine| machine.started_at);
        VirtualMachinesReadout { machines }
    }
}

impl SnapshotSource for VirtualMachinesReadout {
    type Error = ChangeWatchFailure;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Virtual machines";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system_bus = buses.system;
        let sampler = Arc::new(AsyncMutex::new(VirtualMachineSampler::new(
            system_bus.clone(),
        )));
        triggered_deliveries_with_recheck(
            machine_set_changes(system_bus),
            TRIGGER_BURST_GAP,
            PeriodicPolling::every(VIRTUAL_MACHINE_POLL_PERIOD),
            move || {
                let sampler = sampler.clone();
                async move { Ok(sampler.lock().await.sample().await) }
            },
            |_| SamplingNeed::KeepSampling,
        )
        .repeats_as_unchanged()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qemu_process_of(pid: i32, owner_uid: u32) -> QemuProcess {
        QemuProcess {
            pid,
            owner_uid,
            command_line: vec!["qemu-system-x86_64".to_owned()],
            started_at: SystemTime::UNIX_EPOCH,
            working_directory: ProcessWorkingDirectory::Unreadable,
        }
    }

    #[test]
    fn origins_follow_machined_then_ownership() {
        let libvirt_pids = HashSet::from([30]);
        let known_accounts = KnownAccounts {
            names_by_uid: HashMap::from([
                (30001, "nixbld1".to_owned()),
                (301, "qemu-libvirtd".to_owned()),
            ]),
            subordinate_ranges: vec![SubordinateUidRange {
                owner_name: "forgejo-runner-codeberg".to_owned(),
                first_uid: 200_000,
                uid_count: 65_536,
            }],
        };
        let origin = |pid, owner_uid| {
            origin_of(
                &qemu_process_of(pid, owner_uid),
                &libvirt_pids,
                1000,
                &known_accounts,
            )
        };
        assert_eq!(origin(30, 301), VirtualMachineOrigin::LibvirtDomain);
        assert_eq!(origin(10, 1000), VirtualMachineOrigin::SessionUser);
        assert_eq!(origin(20, 30001), VirtualMachineOrigin::NixosTestBuild);
        assert_eq!(
            origin(40, 301),
            VirtualMachineOrigin::OtherAccount("qemu-libvirtd".to_owned())
        );
        assert_eq!(
            origin(50, 4242),
            VirtualMachineOrigin::OtherAccount("4242".to_owned())
        );
        assert_eq!(
            origin(60, 872_415_232),
            VirtualMachineOrigin::NixosTestBuild
        );
        assert_eq!(
            origin(70, 230_000),
            VirtualMachineOrigin::OtherAccount("a container of forgejo-runner-codeberg".to_owned())
        );
    }
}
