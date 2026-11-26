use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, future::ready, stream};
use zbus::proxy::CacheProperties;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    host_machine_identity::hostname1_os_identity::at_micros,
    login_session_presence::{
        cgroup_inhibitor_app_names::InhibitorAppNames,
        inhibit_reason_text::InhibitReason,
        logind_manager::{InhibitorRow, ManagerProxy},
        wayland_idle_notify::{IdleEdge, input_idle_edges},
    },
    non_empty_text::non_empty,
    system_protocol_clients::{
        dbus_change_signals::{change_triggers, properties_changed_rule, signal_rule},
        dbus_desktop_buses::{BusReadError, DesktopBuses, OnBus},
    },
};

mod kernel_hibernation_checks;

use kernel_hibernation_checks::{read_kernel_hibernation, read_kernel_text};

const LOGIND_SERVICE: &str = "org.freedesktop.login1";
const LOGIND_MANAGER_INTERFACE: &str = "org.freedesktop.login1.Manager";
const PREPARE_FOR_SLEEP_MEMBER: &str = "PrepareForSleep";
const RESUME_DEVICE_PATH: &str = "/sys/power/resume";

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

    fn parsed_logind_operation(operation: &str) -> Option<Self> {
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

impl From<&str> for LogindCapability {
    fn from(answer: &str) -> Self {
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
    fn parsed_inhibited_operation(what: &str) -> Option<Self> {
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
    pub reason: InhibitReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledShutdown {
    pub kind: String,
    pub moment: ShutdownMoment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownMoment {
    At(DateTime<Utc>),
    Unreported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownSchedule {
    Scheduled(ScheduledShutdown),
    NotScheduled,
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
    pub shutdown_schedule: ShutdownSchedule,
}

impl SleepObstacles {
    #[must_use]
    pub fn sleep_state_capability(&self, state: SleepState) -> LogindCapability {
        self.sleep_states
            .iter()
            .find(|support| support.state == state)
            .map_or(LogindCapability::Unsupported, |support| support.capability)
    }

    #[must_use]
    pub fn sleep_state_that_runs(&self) -> Option<SleepState> {
        self.sleep_order.iter().copied().find(|state| {
            matches!(
                self.sleep_state_capability(*state),
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

    pub fn inhibitors_blocking(
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
    type Error = BusReadError;
    type StreamInputs = OnBus<Duration>;

    const NAME: &'static str = "Sleep obstacles";

    fn delivery_stream(
        watch: OnBus<Duration>,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(watch, obstacle_changes)
            .map(SnapshotDelivery::unavailable_when_absent)
    }
}

async fn obstacle_changes(
    watch: OnBus<Duration>,
) -> Result<impl Stream<Item = Result<SleepObstacles, BusReadError>> + Send, BusReadError> {
    let OnBus {
        buses,
        inputs: idle_onset_after,
    } = watch;
    let logind_changes = change_triggers(
        &buses.system,
        [
            properties_changed_rule(LOGIND_SERVICE, LOGIND_MANAGER_INTERFACE)?,
            signal_rule(
                LOGIND_SERVICE,
                LOGIND_MANAGER_INTERFACE,
                PREPARE_FOR_SLEEP_MEMBER,
            )?,
        ],
    )
    .await?;
    let idle_onsets = input_idle_edges(idle_onset_after)
        .filter(|edge| ready(*edge == IdleEdge::Idled))
        .map(|_| Ok(()));
    let readers = prepare_readers(buses).await?;
    Ok(stream::select(logind_changes, idle_onsets)
        .triggered_rereads(move || read_obstacles(readers.clone())))
}

async fn prepare_readers(buses: DesktopBuses) -> Result<ObstacleReaders, BusReadError> {
    Ok(ObstacleReaders {
        manager: ManagerProxy::builder(&buses.system)
            .cache_properties(CacheProperties::No)
            .build()
            .await?,
        app_names: Arc::new(InhibitorAppNames::load().await),
    })
}

async fn read_obstacles(readers: ObstacleReaders) -> Result<SleepObstacles, BusReadError> {
    let ObstacleReaders { manager, app_names } = readers;
    let (shutdown_kind, shutdown_usec) = manager.scheduled_shutdown().await?;
    let sleep_states = vec![
        SleepStateSupport {
            state: SleepState::Suspend,
            capability: LogindCapability::from(manager.can_suspend().await?.as_str()),
        },
        SleepStateSupport {
            state: SleepState::Hibernate,
            capability: LogindCapability::from(manager.can_hibernate().await?.as_str()),
        },
        SleepStateSupport {
            state: SleepState::HybridSleep,
            capability: LogindCapability::from(manager.can_hybrid_sleep().await?.as_str()),
        },
        SleepStateSupport {
            state: SleepState::SuspendThenHibernate,
            capability: LogindCapability::from(
                manager.can_suspend_then_hibernate().await?.as_str(),
            ),
        },
    ];
    Ok(SleepObstacles {
        sleep_states,
        sleep_order: manager
            .sleep_operation()
            .await?
            .iter()
            .filter_map(|operation| SleepState::parsed_logind_operation(operation))
            .collect(),
        kernel_hibernation: read_kernel_hibernation(&app_names).await,
        resume_device: ResumeDevice::from(read_kernel_text(RESUME_DEVICE_PATH).as_str()),
        blocking_inhibitors: blocking_inhibitors(manager.list_inhibitors().await?, &app_names),
        idle_action_setting: if manager.idle_action().await? == "ignore" {
            IdleActionSetting::Ignored
        } else {
            IdleActionSetting::Acting
        },
        shutdown_schedule: match non_empty(shutdown_kind) {
            Some(kind) => ShutdownSchedule::Scheduled(ScheduledShutdown {
                kind,
                moment: at_micros(shutdown_usec)
                    .map_or(ShutdownMoment::Unreported, ShutdownMoment::At),
            }),
            None => ShutdownSchedule::NotScheduled,
        },
    })
}

fn blocking_inhibitors(
    rows: Vec<InhibitorRow>,
    app_names: &InhibitorAppNames,
) -> Vec<BlockingInhibitor> {
    rows.into_iter()
        .filter(|row| matches!(row.mode.as_str(), "block" | "block-weak"))
        .flat_map(|row| {
            let holder = app_names.lock_holder_app_name(&row.who, row.pid);
            let reason = InhibitReason::from(row.why);
            row.what
                .split(':')
                .filter_map(InhibitedOperation::parsed_inhibited_operation)
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

#[cfg(test)]
mod sleep_obstacle_tests;
