use std::{cmp::Reverse, time::Duration};

use futures::{
    Stream, StreamExt, TryStreamExt,
    future::{join_all, ready},
    stream,
};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{Ratio, Time},
    ratio::{percent, ratio},
};
use zbus::proxy::CacheProperties;

use super::{ManagerProxy, UnitRow, manager_signal, unit_proxy};
use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking,
    system_protocol_clients::{
        dbus_change_signals::change_triggers, dbus_desktop_buses::DesktopBuses,
    },
    systemd_units_journal::journal_startup_history::read_previous_os_startup,
};

const SLOWEST_UNIT_COUNT: usize = 10;
const SHORTEST_REPORTED_ACTIVATION: Duration = Duration::from_millis(100);
const DEVICE_UNIT_SUFFIX: &str = ".device";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootPhases {
    pub firmware: Option<Duration>,
    pub loader: Option<Duration>,
    pub kernel: Duration,
    pub initrd: Option<Duration>,
    pub userspace: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStage {
    Firmware,
    Loader,
    Kernel,
    Initrd,
    Userspace,
}

impl BootStage {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            BootStage::Firmware => "firmware",
            BootStage::Loader => "loader",
            BootStage::Kernel => "kernel",
            BootStage::Initrd => "initrd",
            BootStage::Userspace => "userspace",
        }
    }

    #[must_use]
    pub fn meaning(self) -> &'static str {
        match self {
            BootStage::Firmware => "UEFI, before the boot loader",
            BootStage::Loader => "boot loader picking and loading the kernel",
            BootStage::Kernel => "kernel until it hands over to initrd or userspace",
            BootStage::Initrd => "early userspace that mounts the root file system",
            BootStage::Userspace => "systemd starting units until the default target",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootPhase {
    pub stage: BootStage,
    pub duration: Duration,
}

impl BootPhases {
    fn of_timestamps(timestamps: &ManagerTimestamps, finish: Duration) -> Option<Self> {
        let userspace_start = timestamps.userspace?;
        let loader_start = timestamps.loader;
        Some(Self {
            firmware: timestamps.firmware.map(|firmware_start| {
                loader_start.map_or(firmware_start, |loader_start| {
                    firmware_start.saturating_sub(loader_start)
                })
            }),
            loader: loader_start,
            kernel: timestamps.initrd.unwrap_or(userspace_start),
            initrd: timestamps
                .initrd
                .map(|initrd_start| userspace_start.saturating_sub(initrd_start)),
            userspace: finish.saturating_sub(userspace_start),
        })
    }

    #[must_use]
    pub fn present(&self) -> Vec<BootPhase> {
        [
            (BootStage::Firmware, self.firmware),
            (BootStage::Loader, self.loader),
            (BootStage::Kernel, Some(self.kernel)),
            (BootStage::Initrd, self.initrd),
            (BootStage::Userspace, Some(self.userspace)),
        ]
        .into_iter()
        .filter_map(|(stage, duration)| duration.map(|duration| BootPhase { stage, duration }))
        .collect()
    }

    #[must_use]
    pub fn total(&self) -> Duration {
        self.present().iter().map(|phase| phase.duration).sum()
    }

    #[must_use]
    pub fn os_startup(&self) -> OsStartupPhases {
        OsStartupPhases {
            kernel: self.kernel,
            initrd: self.initrd,
            userspace: self.userspace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsStartupPhases {
    pub kernel: Duration,
    pub initrd: Option<Duration>,
    pub userspace: Duration,
}

impl OsStartupPhases {
    #[must_use]
    pub fn total(&self) -> Duration {
        self.kernel + self.initrd.unwrap_or_default() + self.userspace
    }

    fn stage_duration(&self, stage: BootStage) -> Option<Duration> {
        match stage {
            BootStage::Firmware | BootStage::Loader => None,
            BootStage::Kernel => Some(self.kernel),
            BootStage::Initrd => self.initrd,
            BootStage::Userspace => Some(self.userspace),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StartupSlowdownThresholds {
    pub warning_percent: u32,
    pub critical_percent: u32,
}

const SHORTEST_NOTICEABLE_SLOWDOWN: Duration = Duration::from_secs(1);

impl StartupSlowdownThresholds {
    fn severity(self, slowdown: Duration, change: Ratio) -> Severity {
        if slowdown < SHORTEST_NOTICEABLE_SLOWDOWN {
            Severity::Normal
        } else if change >= Ratio::new::<percent>(f64::from(self.critical_percent)) {
            Severity::Critical
        } else if change >= Ratio::new::<percent>(f64::from(self.warning_percent)) {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DurationComparison {
    pub previous: Duration,
    pub change: Ratio,
    pub severity: Severity,
}

impl DurationComparison {
    fn of(
        current: Duration,
        previous: Duration,
        thresholds: StartupSlowdownThresholds,
    ) -> Option<Self> {
        let previous_time = Time::try_from(previous).ok()?;
        let current_time = Time::try_from(current).ok()?;
        let change: Ratio = current_time / previous_time;
        let change = Some(change - Ratio::new::<ratio>(1.0)).filter(|change| change.is_finite())?;
        Some(Self {
            previous,
            change,
            severity: thresholds.severity(current.saturating_sub(previous), change),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageComparison {
    pub stage: BootStage,
    pub comparison: DurationComparison,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartupComparison {
    pub os_startup: DurationComparison,
    pub stages: Vec<StageComparison>,
}

impl StartupComparison {
    fn of(
        current: OsStartupPhases,
        previous: OsStartupPhases,
        thresholds: StartupSlowdownThresholds,
    ) -> Option<Self> {
        let stages = [BootStage::Kernel, BootStage::Initrd, BootStage::Userspace]
            .into_iter()
            .filter_map(|stage| {
                Some(StageComparison {
                    stage,
                    comparison: DurationComparison::of(
                        current.stage_duration(stage)?,
                        previous.stage_duration(stage)?,
                        thresholds,
                    )?,
                })
            })
            .collect();
        Some(Self {
            os_startup: DurationComparison::of(current.total(), previous.total(), thresholds)?,
            stages,
        })
    }

    #[must_use]
    pub fn stage(&self, stage: BootStage) -> Option<&DurationComparison> {
        self.stages
            .iter()
            .find(|stage_comparison| stage_comparison.stage == stage)
            .map(|stage_comparison| &stage_comparison.comparison)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitActivation {
    pub name: String,
    pub duration: Duration,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedActivations {
    pub slowest_first: Vec<UnitActivation>,
}

impl RankedActivations {
    #[must_use]
    pub fn slow_count(&self) -> usize {
        self.slowest_first
            .iter()
            .filter(|activation| activation.severity > Severity::Normal)
            .count()
    }
}

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
    pub user_startup: Option<UserStartup>,
    pub startup_comparison: Option<StartupComparison>,
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
                                .inspect_err(|e| tracing::warn!("systemd user startup: {e}"))
                                .ok()
                                .and_then(UserStartup::of),
                        )
                    })
                    .map({
                        let boot = boot.clone();
                        move |user_startup| BootTimeline {
                            user_startup: Some(user_startup),
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
    let phases = BootPhases::of_timestamps(&startup.timestamps, startup.finish)
        .ok_or_else(|| zbus::Error::Failure("systemd reports no userspace start".to_owned()))?;
    let previous_os_startup = blocking("systemd startup history", || {
        read_previous_os_startup()
            .inspect_err(|e| tracing::warn!("systemd startup history: {e}"))
            .unwrap_or_default()
    })
    .await;
    Ok(BootTimeline {
        startup_comparison: previous_os_startup.and_then(|previous| {
            StartupComparison::of(phases.os_startup(), previous, slowdown_thresholds)
        }),
        phases,
        system_startup: SystemStartup {
            finish: startup.finish,
            slowest_units: startup.slowest_units,
        },
        user_startup: None,
    })
}

impl UserStartup {
    fn of(startup: ManagerStartup) -> Option<Self> {
        let started_at = startup.timestamps.userspace?;
        Some(Self {
            started_at,
            duration: startup.finish.checked_sub(started_at)?,
            slowest_units: startup.slowest_units,
        })
    }
}

struct ManagerTimestamps {
    firmware: Option<Duration>,
    loader: Option<Duration>,
    initrd: Option<Duration>,
    userspace: Option<Duration>,
    finish: Option<Duration>,
}

impl ManagerTimestamps {
    async fn read(manager: &ManagerProxy<'_>) -> zbus::Result<Self> {
        Ok(Self {
            firmware: since_boot(manager.firmware_timestamp_monotonic().await?),
            loader: since_boot(manager.loader_timestamp_monotonic().await?),
            initrd: since_boot(manager.initrd_timestamp_monotonic().await?),
            userspace: since_boot(manager.userspace_timestamp_monotonic().await?),
            finish: since_boot(manager.finish_timestamp_monotonic().await?),
        })
    }
}

fn since_boot(monotonic_us: u64) -> Option<Duration> {
    (monotonic_us > 0).then(|| Duration::from_micros(monotonic_us))
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
        if let Some(finish) = timestamps.finish {
            break finish;
        }
        startup_finished
            .next()
            .await
            .ok_or_else(|| zbus::Error::Failure("startup signal stream ended".to_owned()))?;
        timestamps = ManagerTimestamps::read(&manager).await?;
    };
    let slowest_units = slowest_activations(&manager, finish, slow_unit).await?;
    Ok(ManagerStartup {
        timestamps,
        finish,
        slowest_units,
    })
}

struct ActivationStamps {
    name: String,
    inactive_exit: Option<Duration>,
    active_enter: Option<Duration>,
}

async fn slowest_activations(
    manager: &ManagerProxy<'_>,
    finish: Duration,
    slow_unit: Duration,
) -> zbus::Result<RankedActivations> {
    let connection = manager.inner().connection();
    let rows = manager.list_units().await?;
    let stamps = join_all(
        rows.into_iter()
            .filter(|row| !row.name.ends_with(DEVICE_UNIT_SUFFIX))
            .map(|row| activation_stamps(connection, row)),
    )
    .await;
    Ok(slowest_before_finish(
        stamps.into_iter().flatten(),
        finish,
        slow_unit,
    ))
}

async fn activation_stamps(
    connection: &zbus::Connection,
    row: UnitRow,
) -> Option<ActivationStamps> {
    let unit = unit_proxy(connection, row.unit_path).await.ok()?;
    Some(ActivationStamps {
        inactive_exit: since_boot(unit.inactive_exit_timestamp_monotonic().await.ok()?),
        active_enter: since_boot(unit.active_enter_timestamp_monotonic().await.ok()?),
        name: row.name,
    })
}

fn slowest_before_finish(
    stamps: impl IntoIterator<Item = ActivationStamps>,
    finish: Duration,
    slow_unit: Duration,
) -> RankedActivations {
    let mut activations: Vec<UnitActivation> = stamps
        .into_iter()
        .filter_map(|stamps| {
            let active_enter = stamps.active_enter.filter(|entered| *entered <= finish)?;
            let duration = active_enter
                .checked_sub(stamps.inactive_exit?)
                .filter(|duration| *duration >= SHORTEST_REPORTED_ACTIVATION)?;
            Some(UnitActivation {
                name: stamps.name,
                duration,
                severity: if duration >= slow_unit {
                    Severity::Warning
                } else {
                    Severity::Normal
                },
            })
        })
        .collect();
    activations.sort_by_key(|activation| Reverse(activation.duration));
    activations.truncate(SLOWEST_UNIT_COUNT);
    RankedActivations {
        slowest_first: activations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(seconds: f64) -> Duration {
        Duration::from_secs_f64(seconds)
    }

    const THRESHOLDS: StartupSlowdownThresholds = StartupSlowdownThresholds {
        warning_percent: 25,
        critical_percent: 60,
    };

    fn assert_whole_percent(change: Ratio, expected_percent: f64) {
        assert!(
            (change.get::<percent>() - expected_percent).abs() < 0.5,
            "{} % is not about {expected_percent} %",
            change.get::<percent>()
        );
    }

    fn os_phases(kernel: f64, initrd: Option<f64>, userspace: f64) -> OsStartupPhases {
        OsStartupPhases {
            kernel: seconds(kernel),
            initrd: initrd.map(seconds),
            userspace: seconds(userspace),
        }
    }

    #[test]
    fn startup_compares_only_stages_both_boots_had() {
        let comparison = StartupComparison::of(
            os_phases(0.5, Some(6.0), 12.0),
            os_phases(0.6, None, 10.0),
            THRESHOLDS,
        )
        .expect("a comparison");
        assert_eq!(comparison.os_startup.previous.as_millis(), 10_600);
        assert_whole_percent(comparison.os_startup.change, 74.5);
        assert_eq!(comparison.os_startup.severity, Severity::Critical);
        assert_eq!(comparison.stage(BootStage::Initrd), None);
        assert_eq!(comparison.stage(BootStage::Firmware), None);
        let userspace = comparison
            .stage(BootStage::Userspace)
            .expect("userspace compared");
        assert_whole_percent(userspace.change, 20.0);
        assert_eq!(userspace.severity, Severity::Normal);
    }

    #[test]
    fn comparison_grades_by_thresholds_and_ignores_sub_second_slowdowns() {
        let graded = |current: f64| {
            DurationComparison::of(seconds(current), seconds(10.0), THRESHOLDS)
                .expect("a comparison")
        };
        assert_whole_percent(graded(9.0).change, -10.0);
        assert_eq!(graded(12.4).severity, Severity::Normal);
        assert_eq!(graded(12.5).severity, Severity::Warning);
        assert_eq!(graded(15.9).severity, Severity::Warning);
        assert_eq!(graded(16.0).severity, Severity::Critical);
        let tiny =
            DurationComparison::of(seconds(0.9), seconds(0.5), THRESHOLDS).expect("a comparison");
        assert_whole_percent(tiny.change, 80.0);
        assert_eq!(tiny.severity, Severity::Normal);
        assert_eq!(
            DurationComparison::of(seconds(1.0), Duration::ZERO, THRESHOLDS),
            None
        );
    }

    #[test]
    fn phases_follow_systemd_analyze_and_skip_a_missing_loader() {
        let timestamps = ManagerTimestamps {
            firmware: Some(seconds(26.0)),
            loader: None,
            initrd: Some(seconds(0.5)),
            userspace: Some(seconds(4.0)),
            finish: Some(seconds(37.0)),
        };
        let phases =
            BootPhases::of_timestamps(&timestamps, Duration::from_secs(37)).expect("phases");
        assert_eq!(
            phases.present(),
            [
                BootPhase {
                    stage: BootStage::Firmware,
                    duration: Duration::from_secs(26),
                },
                BootPhase {
                    stage: BootStage::Kernel,
                    duration: Duration::from_millis(500),
                },
                BootPhase {
                    stage: BootStage::Initrd,
                    duration: Duration::from_millis(3500),
                },
                BootPhase {
                    stage: BootStage::Userspace,
                    duration: Duration::from_secs(33),
                },
            ]
        );
    }

    #[test]
    fn blame_skips_instant_units_and_units_activated_after_boot() {
        let stamps = |name: &str, began: f64, entered: f64| ActivationStamps {
            name: name.to_owned(),
            inactive_exit: Some(seconds(began)),
            active_enter: Some(seconds(entered)),
        };
        let slowest = slowest_before_finish(
            [
                stamps("fstrim.service", 3600.0, 3950.0),
                stamps("systemd-rfkill.service", 4.0, 32.0),
                stamps("dbus.service", 6.0, 6.5),
                stamps("dbus.socket", 6.0, 6.01),
            ],
            Duration::from_secs(37),
            Duration::from_secs(5),
        );
        let names: Vec<(&str, Severity)> = slowest
            .slowest_first
            .iter()
            .map(|activation| (activation.name.as_str(), activation.severity))
            .collect();
        assert_eq!(
            names,
            [
                ("systemd-rfkill.service", Severity::Warning),
                ("dbus.service", Severity::Normal)
            ]
        );
    }
}
