mod boot_stage_phases;
mod startup_comparison;
mod unit_activation_ranking;

use std::time::Duration;

use futures::{Stream, StreamExt, TryStreamExt, future::ready, stream};
use zbus::proxy::CacheProperties;

pub use self::{
    boot_stage_phases::{BootPhase, BootPhases, BootStage, InitrdPhase, OsStartupPhases},
    startup_comparison::{
        DurationComparison, StageComparison, StartupComparison, StartupSlowdownThresholds,
    },
    unit_activation_ranking::{RankedActivations, UnitActivation},
};
use super::{ManagerProxy, manager_signal};
use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking,
    system_protocol_clients::{
        dbus_change_signals::change_triggers, dbus_desktop_buses::DesktopBuses,
    },
    systemd_units_journal::{
        boot_timeline::unit_activation_ranking::slowest_activations,
        journal_startup_history::read_previous_os_startup,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemStartup {
    pub finish: Duration,
    pub slowest_units: RankedActivations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStartup {
    pub started_at: Duration,
    pub duration: Duration,
    pub slowest_units: RankedActivations,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BootTimeline {
    pub phases: BootPhases,
    pub system_startup: SystemStartup,
    pub user_startup: UserStartupProgress,
    pub startup_comparison: PreviousBootComparison,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserStartupProgress {
    AwaitingLogin,
    Finished(UserStartup),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreviousBootComparison {
    NoPreviousBoot,
    Compared(StartupComparison),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BootTimelineWatch {
    pub buses: DesktopBuses,
    pub slow_unit: Duration,
    pub slowdown_thresholds: StartupSlowdownThresholds,
}

impl SnapshotSource for BootTimeline {
    type Error = zbus::Error;
    type StreamInputs = BootTimelineWatch;

    const NAME: &'static str = "Boot timeline";

    fn delivery_stream(
        watch: BootTimelineWatch,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let BootTimelineWatch {
            buses,
            slow_unit,
            slowdown_thresholds,
        } = watch;
        stream::once(system_boot(
            buses.system.clone(),
            slow_unit,
            slowdown_thresholds,
        ))
        .map_ok(move |boot| {
            let with_user_startup =
                stream::once(finished_startup(buses.session.clone(), slow_unit))
                    .filter_map(|startup| {
                        ready(
                            startup
                                .inspect_err(|error| {
                                    tracing::warn!("systemd user startup: {error}");
                                })
                                .ok()
                                .and_then(UserStartup::measured_startup),
                        )
                    })
                    .map({
                        let boot = boot.clone();
                        move |user_startup| BootTimeline {
                            user_startup: UserStartupProgress::Finished(user_startup),
                            ..boot.clone()
                        }
                    });
            stream::iter([boot]).chain(with_user_startup).map(Ok)
        })
        .try_flatten()
        .map(SnapshotDelivery::live_push)
    }
}

async fn system_boot(
    system: zbus::Connection,
    slow_unit: Duration,
    slowdown_thresholds: StartupSlowdownThresholds,
) -> zbus::Result<BootTimeline> {
    let startup = finished_startup(system, slow_unit).await?;
    let phases = BootPhases::phases_until_finish(&startup.timestamps, startup.finish)
        .ok_or_else(|| zbus::Error::Failure("systemd reports no userspace start".to_owned()))?;
    let previous_os_startup = blocking("systemd startup history", || {
        read_previous_os_startup().unwrap_or_else(|history_failure| {
            tracing::warn!("systemd startup history: {history_failure}");
            None
        })
    })
    .await;
    Ok(BootTimeline {
        startup_comparison: previous_os_startup
            .and_then(|previous| {
                StartupComparison::compared_startups(
                    phases.os_startup,
                    previous,
                    slowdown_thresholds,
                )
            })
            .map_or(
                PreviousBootComparison::NoPreviousBoot,
                PreviousBootComparison::Compared,
            ),
        phases,
        system_startup: SystemStartup {
            finish: startup.finish,
            slowest_units: startup.slowest_units,
        },
        user_startup: UserStartupProgress::AwaitingLogin,
    })
}

impl UserStartup {
    fn measured_startup(startup: ManagerStartup) -> Option<Self> {
        let MonotonicStamp::SinceBoot(started_at) = startup.timestamps.userspace else {
            return None;
        };
        Some(Self {
            started_at,
            duration: startup.finish.checked_sub(started_at)?,
            slowest_units: startup.slowest_units,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonotonicStamp {
    Unset,
    SinceBoot(Duration),
}

impl From<u64> for MonotonicStamp {
    fn from(monotonic_usec: u64) -> Self {
        match monotonic_usec {
            0 => MonotonicStamp::Unset,
            since_boot_usec => MonotonicStamp::SinceBoot(Duration::from_micros(since_boot_usec)),
        }
    }
}

struct ManagerTimestamps {
    firmware: MonotonicStamp,
    loader: MonotonicStamp,
    initrd: MonotonicStamp,
    userspace: MonotonicStamp,
    finish: MonotonicStamp,
}

impl ManagerTimestamps {
    async fn read(manager: &ManagerProxy<'_>) -> zbus::Result<Self> {
        Ok(Self {
            firmware: manager.firmware_timestamp_monotonic().await?.into(),
            loader: manager.loader_timestamp_monotonic().await?.into(),
            initrd: manager.initrd_timestamp_monotonic().await?.into(),
            userspace: manager.userspace_timestamp_monotonic().await?.into(),
            finish: manager.finish_timestamp_monotonic().await?.into(),
        })
    }
}

struct ManagerStartup {
    timestamps: ManagerTimestamps,
    finish: Duration,
    slowest_units: RankedActivations,
}

async fn finished_startup(
    connection: zbus::Connection,
    slow_unit: Duration,
) -> zbus::Result<ManagerStartup> {
    let manager = ManagerProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let mut startup_finished =
        change_triggers(&connection, [manager_signal("StartupFinished")?]).await?;
    manager.ensure_subscribed().await?;
    let mut timestamps = ManagerTimestamps::read(&manager).await?;
    let finish = loop {
        if let MonotonicStamp::SinceBoot(finish) = timestamps.finish {
            break finish;
        }
        startup_finished
            .next()
            .await
            .ok_or_else(|| zbus::Error::Failure("startup signal stream ended".to_owned()))??;
        timestamps = ManagerTimestamps::read(&manager).await?;
    };
    let slowest_units = slowest_activations(&manager, finish, slow_unit).await?;
    Ok(ManagerStartup {
        timestamps,
        finish,
        slowest_units,
    })
}
