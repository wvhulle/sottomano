use std::{fs, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use futures::Stream;
use tokio::{process::Command, task::spawn_blocking};
use zbus::proxy::CacheProperties;

use crate::{
    SnapshotDelivery, SnapshotSource,
    compositor_window_ipc::compositor_connection::compositor_window_pids,
    delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::process_tree_inspection::launch,
    host_machine_identity::hostname1_os_identity::at_micros,
    login_session_presence::{
        cgroup_inhibitor_app_names::InhibitorAppNames,
        logind_manager::{InhibitorRow, ManagerProxy},
    },
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::{self, DesktopBuses},
};

const OBSTACLE_POLL_PERIOD: Duration = Duration::from_secs(30);
const HIBERNATION_MODES_PATH: &str = "/sys/power/disk";
const RESUME_DEVICE_PATH: &str = "/sys/power/resume";
const KERNEL_COMMAND_LINE_PATH: &str = "/proc/cmdline";
const KERNEL_LOCKDOWN_PATH: &str = "/sys/kernel/security/lockdown";
const SECRET_MEMORY_HELPER_PATH: &str = "/run/wrappers/bin/sottomano-secret-memory-holders";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepState {
    Suspend,
    Hibernate,
    HybridSleep,
    SuspendThenHibernate,
}

impl SleepState {
    pub const ALL: [SleepState; 4] = [
        Self::Suspend,
        Self::Hibernate,
        Self::HybridSleep,
        Self::SuspendThenHibernate,
    ];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            SleepState::Suspend => "Suspend",
            SleepState::Hibernate => "Hibernate",
            SleepState::HybridSleep => "Hybrid sleep",
            SleepState::SuspendThenHibernate => "Suspend, then hibernate",
        }
    }

    fn from_logind_operation(operation: &str) -> Option<Self> {
        match operation {
            "suspend" => Some(Self::Suspend),
            "hibernate" => Some(Self::Hibernate),
            "hybrid-sleep" => Some(Self::HybridSleep),
            "suspend-then-hibernate" => Some(Self::SuspendThenHibernate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelSleepVerdict {
    Permitted,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogindCapability {
    Allowed,
    AfterAuthorisation,
    NotPermitted,
    Unsupported,
}

impl LogindCapability {
    fn from_answer(answer: &str) -> Self {
        match answer {
            "yes" => Self::Allowed,
            "challenge" => Self::AfterAuthorisation,
            "no" => Self::NotPermitted,
            _ => Self::Unsupported,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelHibernation {
    Allowed,
    DisabledByBootFlag(String),
    DisabledByLockdown(String),
    DisabledBySecretMemory(Vec<SecretMemoryHolder>),
    DisabledForUnidentifiedCause,
    DisabledWithoutInspection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMemoryHolder {
    pub name: String,
    pub pid: u32,
    pub window_ownership: WindowOwnership,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InhibitedOperation {
    Sleep,
    Shutdown,
    Idle,
}

impl InhibitedOperation {
    fn from_logind_what(what: &str) -> Option<Self> {
        match what {
            "sleep" => Some(Self::Sleep),
            "shutdown" => Some(Self::Shutdown),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingInhibitor {
    pub holder: String,
    pub operation: InhibitedOperation,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledShutdown {
    pub kind: String,
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepStateSupport {
    pub state: SleepState,
    pub capability: LogindCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SleepObstacles {
    pub sleep_states: Vec<SleepStateSupport>,
    pub sleep_order: Vec<SleepState>,
    pub kernel_hibernation: KernelHibernation,
    pub resume_device: ResumeDevice,
    pub blocking_inhibitors: Vec<BlockingInhibitor>,
    pub idle_action_setting: IdleActionSetting,
    pub scheduled_shutdown: Option<ScheduledShutdown>,
}

impl SleepObstacles {
    #[must_use]
    pub fn capability_of(&self, state: SleepState) -> LogindCapability {
        self.sleep_states
            .iter()
            .find(|support| support.state == state)
            .map_or(LogindCapability::Unsupported, |support| support.capability)
    }

    #[must_use]
    pub fn sleep_state_that_runs(&self) -> Option<SleepState> {
        self.sleep_order.iter().copied().find(|state| {
            matches!(
                self.capability_of(*state),
                LogindCapability::Allowed | LogindCapability::AfterAuthorisation
            )
        })
    }

    #[must_use]
    pub fn kernel_verdict(&self, state: SleepState) -> KernelSleepVerdict {
        match (state, &self.kernel_hibernation) {
            (SleepState::Suspend, _) | (_, KernelHibernation::Allowed) => {
                KernelSleepVerdict::Permitted
            }
            (
                SleepState::Hibernate | SleepState::HybridSleep | SleepState::SuspendThenHibernate,
                _,
            ) => KernelSleepVerdict::Blocked,
        }
    }

    pub fn inhibitors_of(
        &self,
        operation: InhibitedOperation,
    ) -> impl Iterator<Item = &BlockingInhibitor> {
        self.blocking_inhibitors
            .iter()
            .filter(move |inhibitor| inhibitor.operation == operation)
    }
}

#[derive(Clone)]
struct ObstacleReaders {
    manager: ManagerProxy<'static>,
    app_names: Arc<InhibitorAppNames>,
}

impl SnapshotSource for SleepObstacles {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Sleep obstacles";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(OBSTACLE_POLL_PERIOD)
            .poll_deliveries_after_setup(prepare_readers(buses), read_obstacles)
    }
}

async fn prepare_readers(
    buses: DesktopBuses,
) -> Result<ObstacleReaders, dbus_desktop_buses::Error> {
    Ok(ObstacleReaders {
        manager: ManagerProxy::builder(&buses.system)
            .cache_properties(CacheProperties::No)
            .build()
            .await?,
        app_names: Arc::new(InhibitorAppNames::load().await),
    })
}

async fn read_obstacles(
    readers: ObstacleReaders,
) -> Result<SleepObstacles, dbus_desktop_buses::Error> {
    let ObstacleReaders { manager, app_names } = readers;
    let (shutdown_kind, shutdown_usec) = manager.scheduled_shutdown().await?;
    let sleep_states = vec![
        SleepStateSupport {
            state: SleepState::Suspend,
            capability: LogindCapability::from_answer(&manager.can_suspend().await?),
        },
        SleepStateSupport {
            state: SleepState::Hibernate,
            capability: LogindCapability::from_answer(&manager.can_hibernate().await?),
        },
        SleepStateSupport {
            state: SleepState::HybridSleep,
            capability: LogindCapability::from_answer(&manager.can_hybrid_sleep().await?),
        },
        SleepStateSupport {
            state: SleepState::SuspendThenHibernate,
            capability: LogindCapability::from_answer(&manager.can_suspend_then_hibernate().await?),
        },
    ];
    Ok(SleepObstacles {
        sleep_states,
        sleep_order: manager
            .sleep_operation()
            .await?
            .iter()
            .filter_map(|operation| SleepState::from_logind_operation(operation))
            .collect(),
        kernel_hibernation: read_kernel_hibernation(&app_names).await,
        resume_device: ResumeDevice::from(read_kernel_text(RESUME_DEVICE_PATH).as_str()),
        blocking_inhibitors: blocking_inhibitors(manager.list_inhibitors().await?, &app_names),
        idle_action_setting: if manager.idle_action().await? == "ignore" {
            IdleActionSetting::Ignored
        } else {
            IdleActionSetting::Acting
        },
        scheduled_shutdown: non_empty(shutdown_kind).map(|kind| ScheduledShutdown {
            kind,
            at: at_micros(shutdown_usec),
        }),
    })
}

fn read_kernel_text(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

async fn read_kernel_hibernation(app_names: &InhibitorAppNames) -> KernelHibernation {
    if HibernationModes::from(read_kernel_text(HIBERNATION_MODES_PATH).as_str())
        == HibernationModes::Offered
    {
        return KernelHibernation::Allowed;
    }
    if let Some(flag) = disabling_boot_flag(&read_kernel_text(KERNEL_COMMAND_LINE_PATH)) {
        return KernelHibernation::DisabledByBootFlag(flag);
    }
    if let Some(level) = active_lockdown_level(&read_kernel_text(KERNEL_LOCKDOWN_PATH)) {
        return KernelHibernation::DisabledByLockdown(level);
    }
    match secret_memory_scan().await {
        SecretMemoryScan::Unavailable => KernelHibernation::DisabledWithoutInspection,
        SecretMemoryScan::Scanned(pids) if pids.is_empty() => {
            KernelHibernation::DisabledForUnidentifiedCause
        }
        SecretMemoryScan::Scanned(pids) => KernelHibernation::DisabledBySecretMemory(
            secret_memory_holders(&pids, app_names, &open_window_owners().await),
        ),
    }
}

enum SecretMemoryScan {
    Scanned(Vec<u32>),
    Unavailable,
}

async fn secret_memory_scan() -> SecretMemoryScan {
    match Command::new(SECRET_MEMORY_HELPER_PATH).output().await {
        Ok(output) if output.status.success() => SecretMemoryScan::Scanned(pids_in_helper_output(
            &String::from_utf8_lossy(&output.stdout),
        )),
        Ok(_) | Err(_) => SecretMemoryScan::Unavailable,
    }
}

fn secret_memory_holders(
    pids: &[u32],
    app_names: &InhibitorAppNames,
    window_owners: &CompositorWindows,
) -> Vec<SecretMemoryHolder> {
    let mut holders: Vec<SecretMemoryHolder> = pids
        .iter()
        .map(|pid| SecretMemoryHolder {
            name: app_names
                .of_process(*pid)
                .unwrap_or_else(|| format!("Process {pid}")),
            pid: *pid,
            window_ownership: window_owners.window_ownership(*pid),
        })
        .collect();
    holders.sort_by(|earlier, later| {
        (earlier.name.as_str(), earlier.pid).cmp(&(later.name.as_str(), later.pid))
    });
    holders
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompositorWindows {
    Listed(WindowOwners),
    Unavailable,
}

impl CompositorWindows {
    fn window_ownership(&self, pid: u32) -> WindowOwnership {
        match self {
            Self::Listed(owners) => owners.window_ownership(pid, process_scope(pid).as_deref()),
            Self::Unavailable => WindowOwnership::OwnsWindow,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct WindowOwners {
    window_pids: Vec<u32>,
    window_scopes: Vec<String>,
}

impl WindowOwners {
    fn window_ownership(&self, pid: u32, scope: Option<&str>) -> WindowOwnership {
        let owns = self.window_pids.contains(&pid)
            || scope.is_some_and(|scope| self.window_scopes.iter().any(|owned| owned == scope));
        if owns {
            WindowOwnership::OwnsWindow
        } else {
            WindowOwnership::Windowless
        }
    }
}

fn process_scope(pid: u32) -> Option<String> {
    launch(i32::try_from(pid).ok()?)?.scope
}

async fn open_window_owners() -> CompositorWindows {
    let Some(compositor_pids) = compositor_window_pids().await else {
        return CompositorWindows::Unavailable;
    };
    spawn_blocking(move || window_owners_of(compositor_pids))
        .await
        .unwrap_or(CompositorWindows::Unavailable)
}

fn window_owners_of(compositor_pids: Vec<i32>) -> CompositorWindows {
    let window_pids: Vec<u32> = compositor_pids
        .into_iter()
        .filter_map(|pid| u32::try_from(pid).ok())
        .collect();
    let window_scopes = window_pids
        .iter()
        .filter_map(|pid| process_scope(*pid))
        .filter(|scope| scope.starts_with("app-"))
        .collect();
    CompositorWindows::Listed(WindowOwners {
        window_pids,
        window_scopes,
    })
}

fn blocking_inhibitors(
    rows: Vec<InhibitorRow>,
    app_names: &InhibitorAppNames,
) -> Vec<BlockingInhibitor> {
    rows.into_iter()
        .filter(|row| matches!(row.mode.as_str(), "block" | "block-weak"))
        .flat_map(|row| {
            let holder = app_names.of_lock_holder(&row.who, row.pid);
            let reason = non_empty(row.why);
            row.what
                .split(':')
                .filter_map(InhibitedOperation::from_logind_what)
                .map(|operation| BlockingInhibitor {
                    holder: holder.clone(),
                    operation,
                    reason: reason.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HibernationModes {
    Disabled,
    Offered,
}

impl From<&str> for HibernationModes {
    fn from(hibernation_modes: &str) -> Self {
        if hibernation_modes.contains("[disabled]") {
            HibernationModes::Disabled
        } else {
            HibernationModes::Offered
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeDevice {
    Configured,
    Unset,
}

impl From<&str> for ResumeDevice {
    fn from(resume_device: &str) -> Self {
        match resume_device.trim() {
            "" | "0:0" => ResumeDevice::Unset,
            _ => ResumeDevice::Configured,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleActionSetting {
    Acting,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowOwnership {
    OwnsWindow,
    Windowless,
}

fn disabling_boot_flag(command_line: &str) -> Option<String> {
    command_line
        .split_whitespace()
        .find(|argument| matches!(*argument, "nohibernate" | "hibernate=no"))
        .map(str::to_owned)
}

fn active_lockdown_level(lockdown_levels: &str) -> Option<String> {
    lockdown_levels
        .split_whitespace()
        .find_map(|level| level.strip_prefix('[')?.strip_suffix(']'))
        .filter(|level| *level != "none")
        .and_then(|level| non_empty(level.to_owned()))
}

fn pids_in_helper_output(helper_output: &str) -> Vec<u32> {
    helper_output
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        InhibitedOperation, KernelHibernation, KernelSleepVerdict, LogindCapability,
        SleepObstacles, SleepState, SleepStateSupport, WindowOwners, active_lockdown_level,
        disabling_boot_flag, pids_in_helper_output,
    };
    use crate::login_session_presence::kernel_sleep_obstacles::{
        HibernationModes, IdleActionSetting, ResumeDevice, WindowOwnership,
    };

    #[test]
    fn a_holder_owns_a_window_through_its_own_pid_or_its_app_scope() {
        let owners = WindowOwners {
            window_pids: vec![6627],
            window_scopes: vec!["app-bitwarden-6370.scope".to_owned()],
        };
        assert_eq!(
            owners.window_ownership(6627, None),
            WindowOwnership::OwnsWindow
        );
        assert_eq!(
            owners.window_ownership(7365, Some("app-bitwarden-6370.scope")),
            WindowOwnership::OwnsWindow
        );
        assert_eq!(
            owners.window_ownership(9000, Some("app-signal-12.scope")),
            WindowOwnership::Windowless
        );
        assert_eq!(
            owners.window_ownership(9001, None),
            WindowOwnership::Windowless
        );
    }

    fn obstacles_with(capabilities: [LogindCapability; 4]) -> SleepObstacles {
        SleepObstacles {
            sleep_states: SleepState::ALL
                .into_iter()
                .zip(capabilities)
                .map(|(state, capability)| SleepStateSupport { state, capability })
                .collect(),
            sleep_order: vec![
                SleepState::SuspendThenHibernate,
                SleepState::Suspend,
                SleepState::Hibernate,
            ],
            kernel_hibernation: KernelHibernation::Allowed,
            resume_device: ResumeDevice::Configured,
            blocking_inhibitors: Vec::new(),
            idle_action_setting: IdleActionSetting::Ignored,
            scheduled_shutdown: None,
        }
    }

    #[test]
    fn only_the_disabled_marker_counts_as_disabled_hibernation() {
        assert_eq!(
            HibernationModes::from("[disabled]\n"),
            HibernationModes::Disabled
        );
        assert_eq!(
            HibernationModes::from("[platform] shutdown reboot suspend test_resume\n"),
            HibernationModes::Offered
        );
    }

    #[test]
    fn a_zero_resume_device_means_hibernation_was_never_set_up() {
        assert_eq!(ResumeDevice::from("259:2\n"), ResumeDevice::Configured);
        assert_eq!(ResumeDevice::from("0:0\n"), ResumeDevice::Unset);
        assert_eq!(ResumeDevice::from(""), ResumeDevice::Unset);
    }

    #[test]
    fn both_boot_flags_that_disable_hibernation_are_recognised() {
        assert_eq!(
            disabling_boot_flag("quiet nohibernate splash").as_deref(),
            Some("nohibernate")
        );
        assert_eq!(
            disabling_boot_flag("hibernate=no").as_deref(),
            Some("hibernate=no")
        );
        assert_eq!(
            disabling_boot_flag("resume=/dev/nvme0n1p2 hibernate=nocompress"),
            None
        );
    }

    #[test]
    fn only_a_selected_level_other_than_none_is_a_lockdown() {
        assert_eq!(
            active_lockdown_level("[none] integrity confidentiality"),
            None
        );
        assert_eq!(
            active_lockdown_level("none [integrity] confidentiality").as_deref(),
            Some("integrity")
        );
    }

    #[test]
    fn helper_output_is_read_as_one_pid_per_line() {
        assert_eq!(pids_in_helper_output("6370\n7001\n"), vec![6370, 7001]);
        assert_eq!(pids_in_helper_output(""), Vec::<u32>::new());
    }

    #[test]
    fn the_first_available_state_in_logind_order_is_what_runs() {
        let obstacles = obstacles_with([
            LogindCapability::Allowed,
            LogindCapability::Unsupported,
            LogindCapability::Unsupported,
            LogindCapability::Unsupported,
        ]);
        assert_eq!(obstacles.sleep_state_that_runs(), Some(SleepState::Suspend));
    }

    #[test]
    fn a_state_needing_authorisation_still_counts_as_available() {
        let obstacles = obstacles_with([
            LogindCapability::Unsupported,
            LogindCapability::Unsupported,
            LogindCapability::Unsupported,
            LogindCapability::AfterAuthorisation,
        ]);
        assert_eq!(
            obstacles.sleep_state_that_runs(),
            Some(SleepState::SuspendThenHibernate)
        );
    }

    #[test]
    fn nothing_runs_when_no_state_is_available() {
        let obstacles = obstacles_with([LogindCapability::NotPermitted; 4]);
        assert_eq!(obstacles.sleep_state_that_runs(), None);
        assert_eq!(
            obstacles.inhibitors_of(InhibitedOperation::Sleep).count(),
            0
        );
    }

    #[test]
    fn the_kernel_only_blocks_states_that_write_a_hibernation_image() {
        let obstacles = SleepObstacles {
            kernel_hibernation: KernelHibernation::DisabledWithoutInspection,
            ..obstacles_with([LogindCapability::Allowed; 4])
        };
        assert_eq!(
            obstacles.kernel_verdict(SleepState::Suspend),
            KernelSleepVerdict::Permitted
        );
        for hibernating_state in [
            SleepState::Hibernate,
            SleepState::HybridSleep,
            SleepState::SuspendThenHibernate,
        ] {
            assert_eq!(
                obstacles.kernel_verdict(hibernating_state),
                KernelSleepVerdict::Blocked
            );
        }
    }
}
