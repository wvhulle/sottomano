use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    future::pending,
    pin::pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, future::select, stream};
use rustix::process::getuid;
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    task::JoinHandle,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking,
    delivery_cadence_drivers::{PeriodicPolling, StampedCadence},
    qemu_virtual_machines::{
        libvirt_domain_control::{LIBVIRT_DOMAIN_CONTROL_BUILT, libvirt_run_states},
        machined_libvirt_machines::{libvirt_leader_pids, machine_set_changes},
        pidfd_process_exits::process_exit,
        procfs_qemu_processes::{
            ProcessWorkingDirectory, QemuProcess, account_names_by_uid, scanned_qemu_processes,
        },
        qemu_command_line_arguments::{QemuLaunchArguments, QmpSocketAccess},
        qmp_monitor_events::{GuestRunState, QmpPowerdownSender, follow_qmp_run_state},
    },
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses,
        procfs_process_tree::{CpuTickCheckpoints, ProcessTreeCpuLoad},
    },
};

pub const VIRTUAL_MACHINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const TRIGGER_BURST_GAP: Duration = Duration::from_millis(200);
const NIX_BUILD_ACCOUNT_PREFIX: &str = "nixbld";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualMachineOrigin {
    SessionUser,
    NixosTestBuild,
    LibvirtDomain,
    OtherAccount(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuestPowerControl {
    QmpSession(QmpPowerdownSender),
    LibvirtDomain(String),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunningVirtualMachine {
    pub pid: i32,
    pub origin: VirtualMachineOrigin,
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

type SharedRunStates = Arc<Mutex<HashMap<i32, GuestRunState>>>;

enum QmpWatch {
    Following(QmpPowerdownSender),
    NotConfigured,
}

struct WatchedMachine {
    watcher: JoinHandle<()>,
    qmp_watch: QmpWatch,
}

fn record_run_state(run_states: &SharedRunStates, pid: i32, run_state: GuestRunState) {
    if let Ok(mut states_by_pid) = run_states.lock() {
        states_by_pid.insert(pid, run_state);
    }
}

fn forget_run_state(run_states: &SharedRunStates, pid: i32) {
    if let Ok(mut states_by_pid) = run_states.lock() {
        states_by_pid.remove(&pid);
    }
}

async fn watch_machine(
    pid: i32,
    qmp_socket: QmpSocketAccess,
    powerdown_requests: mpsc::UnboundedReceiver<()>,
    run_states: SharedRunStates,
    rescan_requests: mpsc::UnboundedSender<()>,
) {
    let follow_qmp = {
        let run_states = run_states.clone();
        let rescan_requests = rescan_requests.clone();
        async move {
            if let Some(socket_path) = qmp_socket.connectable_path() {
                let followed = follow_qmp_run_state(socket_path, powerdown_requests, |run_state| {
                    record_run_state(&run_states, pid, run_state);
                    let _ = rescan_requests.send(());
                })
                .await;
                if let Err(failure) = followed {
                    tracing::debug!("qmp {}: {failure}", socket_path.display());
                }
                forget_run_state(&run_states, pid);
                let _ = rescan_requests.send(());
            }
            pending::<()>().await;
        }
    };
    let exited = async move {
        if let Err(failure) = process_exit(pid).await {
            tracing::debug!("pidfd {pid}: {failure}");
            pending::<()>().await;
        }
    };
    select(pin!(follow_qmp), pin!(exited)).await;
    forget_run_state(&run_states, pid);
    let _ = rescan_requests.send(());
}

fn origin_of(
    qemu_process: &QemuProcess,
    libvirt_pids: &HashSet<i32>,
    session_uid: u32,
    account_names: &HashMap<u32, String>,
) -> VirtualMachineOrigin {
    if libvirt_pids.contains(&qemu_process.pid) {
        return VirtualMachineOrigin::LibvirtDomain;
    }
    if qemu_process.owner_uid == session_uid {
        return VirtualMachineOrigin::SessionUser;
    }
    match account_names.get(&qemu_process.owner_uid) {
        Some(account_name) if account_name.starts_with(NIX_BUILD_ACCOUNT_PREFIX) => {
            VirtualMachineOrigin::NixosTestBuild
        }
        Some(account_name) => VirtualMachineOrigin::OtherAccount(account_name.clone()),
        None => VirtualMachineOrigin::OtherAccount(qemu_process.owner_uid.to_string()),
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

pub(crate) struct VirtualMachineSampler {
    system_bus: zbus::Connection,
    session_uid: u32,
    watched_by_pid: HashMap<i32, WatchedMachine>,
    qmp_run_states: SharedRunStates,
    checkpoints_by_pid: CpuTickCheckpoints<i32>,
    rescan_requests: mpsc::UnboundedSender<()>,
}

impl Drop for VirtualMachineSampler {
    fn drop(&mut self) {
        self.watched_by_pid
            .values()
            .for_each(|watched| watched.watcher.abort());
    }
}

impl VirtualMachineSampler {
    fn new(system_bus: zbus::Connection, rescan_requests: mpsc::UnboundedSender<()>) -> Self {
        Self {
            system_bus,
            session_uid: getuid().as_raw(),
            watched_by_pid: HashMap::new(),
            qmp_run_states: Arc::default(),
            checkpoints_by_pid: CpuTickCheckpoints::default(),
            rescan_requests,
        }
    }

    fn watch_new_machine(&mut self, pid: i32, qmp_socket: &QmpSocketAccess) {
        let (powerdown_sender, powerdown_receiver) = mpsc::unbounded_channel();
        let qmp_watch = match qmp_socket.connectable_path() {
            Some(socket_path) => QmpWatch::Following(QmpPowerdownSender::new(
                socket_path.to_path_buf(),
                powerdown_sender,
            )),
            None => QmpWatch::NotConfigured,
        };
        let watcher = tokio::spawn(watch_machine(
            pid,
            qmp_socket.clone(),
            powerdown_receiver,
            self.qmp_run_states.clone(),
            self.rescan_requests.clone(),
        ));
        self.watched_by_pid
            .insert(pid, WatchedMachine { watcher, qmp_watch });
    }

    fn forget_gone_machines(&mut self, live_pids: &HashSet<i32>) {
        self.watched_by_pid.retain(|pid, watched| {
            let still_running = live_pids.contains(pid);
            if !still_running {
                watched.watcher.abort();
            }
            still_running
        });
        self.checkpoints_by_pid
            .retain_keys(|pid| live_pids.contains(pid));
        if let Ok(mut states_by_pid) = self.qmp_run_states.lock() {
            states_by_pid.retain(|pid, _| live_pids.contains(pid));
        }
    }

    fn qmp_run_state_of(&self, pid: i32) -> Option<GuestRunState> {
        self.qmp_run_states
            .lock()
            .ok()
            .and_then(|states_by_pid| states_by_pid.get(&pid).copied())
    }

    fn power_control_of(&self, identified: &IdentifiedQemuProcess) -> GuestPowerControl {
        if identified.origin == VirtualMachineOrigin::LibvirtDomain && LIBVIRT_DOMAIN_CONTROL_BUILT
        {
            return GuestPowerControl::LibvirtDomain(identified.launch.machine_name.clone());
        }
        let qmp_session_live = self.qmp_run_state_of(identified.qemu_process.pid).is_some();
        match self
            .watched_by_pid
            .get(&identified.qemu_process.pid)
            .map(|watched| &watched.qmp_watch)
        {
            Some(QmpWatch::Following(powerdown_sender)) if qmp_session_live => {
                GuestPowerControl::QmpSession(powerdown_sender.clone())
            }
            _ => GuestPowerControl::Unavailable,
        }
    }

    async fn sample(&mut self) -> VirtualMachinesReadout {
        let scan = blocking("qemu process scan", scanned_qemu_processes).await;
        if scan.qemu_processes.is_empty() {
            self.forget_gone_machines(&HashSet::new());
            return VirtualMachinesReadout::default();
        }
        let libvirt_pids = libvirt_leader_pids(&self.system_bus).await;
        let session_uid = self.session_uid;
        let account_names = if scan
            .qemu_processes
            .iter()
            .any(|qemu_process| qemu_process.owner_uid != session_uid)
        {
            blocking("account names", account_names_by_uid).await
        } else {
            HashMap::new()
        };
        let identified_processes: Vec<IdentifiedQemuProcess> = scan
            .qemu_processes
            .into_iter()
            .map(|qemu_process| IdentifiedQemuProcess {
                launch: launch_arguments_of(&qemu_process),
                origin: origin_of(&qemu_process, &libvirt_pids, session_uid, &account_names),
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
        self.forget_gone_machines(&live_pids);
        for identified in &identified_processes {
            if !self
                .watched_by_pid
                .contains_key(&identified.qemu_process.pid)
            {
                self.watch_new_machine(identified.qemu_process.pid, &identified.launch.qmp_socket);
            }
        }
        let taken_at = Instant::now();
        let mut machines: Vec<RunningVirtualMachine> = identified_processes
            .into_iter()
            .map(|identified| {
                let pid = identified.qemu_process.pid;
                let totals = scan.process_table.tree_totals(pid);
                let run_state = libvirt_states
                    .get(&identified.launch.machine_name)
                    .copied()
                    .filter(|_| identified.origin == VirtualMachineOrigin::LibvirtDomain)
                    .or_else(|| self.qmp_run_state_of(pid))
                    .unwrap_or(GuestRunState::Unreported);
                RunningVirtualMachine {
                    pid,
                    power_control: self.power_control_of(&identified),
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

fn virtual_machine_samples(
    system_bus: zbus::Connection,
) -> impl Stream<Item = VirtualMachinesReadout> + Send {
    let (rescan_sender, rescan_receiver) = mpsc::unbounded_channel();
    let sampler = Arc::new(AsyncMutex::new(VirtualMachineSampler::new(
        system_bus.clone(),
        rescan_sender,
    )));
    let triggers = stream::select(
        UnboundedReceiverStream::new(rescan_receiver),
        machine_set_changes(system_bus),
    )
    .at_most_once_per(TRIGGER_BURST_GAP);
    reread_on_trigger_or_deadline(
        triggers,
        move || {
            let sampler = sampler.clone();
            async move { sampler.lock().await.sample().await }
        },
        |_readout| Some(Instant::now() + VIRTUAL_MACHINE_POLL_PERIOD),
    )
}

impl SnapshotSource for VirtualMachinesReadout {
    type Error = Infallible;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Virtual machines";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        virtual_machine_samples(buses.system)
            .map(|readout| {
                StampedCadence::pushed_with_recheck_while(
                    true,
                    PeriodicPolling::every(VIRTUAL_MACHINE_POLL_PERIOD),
                )
                .delivered(Ok(readout))
            })
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
        let account_names = HashMap::from([
            (30001, "nixbld1".to_owned()),
            (301, "qemu-libvirtd".to_owned()),
        ]);
        let origin = |pid, owner_uid| {
            origin_of(
                &qemu_process_of(pid, owner_uid),
                &libvirt_pids,
                1000,
                &account_names,
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
    }
}
