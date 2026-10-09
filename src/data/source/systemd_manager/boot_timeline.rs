use std::{cmp::Reverse, time::Duration};

use iced::futures::{
    Stream, StreamExt,
    future::{join_all, ready},
    stream,
};
use zbus::proxy::CacheProperties;

use super::{ManagerProxy, UnitRow, connect, manager_signal, signals_matching, unit_proxy};

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
pub struct BootPhase {
    pub name: &'static str,
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

    pub fn present(&self) -> Vec<BootPhase> {
        [
            ("firmware", self.firmware),
            ("loader", self.loader),
            ("kernel", Some(self.kernel)),
            ("initrd", self.initrd),
            ("userspace", Some(self.userspace)),
        ]
        .into_iter()
        .filter_map(|(name, duration)| duration.map(|duration| BootPhase { name, duration }))
        .collect()
    }

    pub fn total(&self) -> Duration {
        self.present().iter().map(|phase| phase.duration).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitActivation {
    pub name: String,
    pub duration: Duration,
    pub slow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedActivations {
    pub slowest_first: Vec<UnitActivation>,
}

impl RankedActivations {
    pub fn slow_count(&self) -> usize {
        self.slowest_first
            .iter()
            .filter(|activation| activation.slow)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemStartup {
    pub slowest_units: RankedActivations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStartup {
    pub duration: Duration,
    pub slowest_units: RankedActivations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootTimeline {
    pub phases: BootPhases,
    pub system_startup: SystemStartup,
    pub user_startup: Option<UserStartup>,
}

pub fn boot_timeline_stream(slow_unit: Duration) -> impl Stream<Item = BootTimeline> {
    stream::once(system_boot(slow_unit))
        .filter_map(|boot| ready(logged("system boot", boot)))
        .flat_map(move |boot| {
            let with_user_startup = stream::once(finished_startup(true, slow_unit))
                .filter_map(|startup| ready(logged("user startup", startup)))
                .filter_map(|startup| ready(UserStartup::of(startup)))
                .map({
                    let boot = boot.clone();
                    move |user_startup| BootTimeline {
                        user_startup: Some(user_startup),
                        ..boot.clone()
                    }
                });
            stream::iter([boot]).chain(with_user_startup)
        })
}

fn logged<T>(stage: &str, reading: zbus::Result<T>) -> Option<T> {
    reading
        .inspect_err(|e| tracing::warn!("systemd {stage}: {e}"))
        .ok()
}

async fn system_boot(slow_unit: Duration) -> zbus::Result<BootTimeline> {
    let startup = finished_startup(false, slow_unit).await?;
    let phases = BootPhases::of_timestamps(&startup.timestamps, startup.finish)
        .ok_or_else(|| zbus::Error::Failure("systemd reports no userspace start".to_owned()))?;
    Ok(BootTimeline {
        phases,
        system_startup: SystemStartup {
            slowest_units: startup.slowest_units,
        },
        user_startup: None,
    })
}

impl UserStartup {
    fn of(startup: ManagerStartup) -> Option<Self> {
        Some(Self {
            duration: startup.finish.checked_sub(startup.timestamps.userspace?)?,
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

async fn finished_startup(user: bool, slow_unit: Duration) -> zbus::Result<ManagerStartup> {
    let connection = connect(user).await?;
    let manager = ManagerProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let mut startup_finished =
        signals_matching(&connection, [manager_signal("StartupFinished")?]).await?;
    if let Err(e) = manager.subscribe().await {
        tracing::warn!("systemd startup: subscribe failed: {e}");
    }
    let mut timestamps = ManagerTimestamps::read(&manager).await?;
    let finish = loop {
        if let Some(finish) = timestamps.finish {
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
                slow: duration >= slow_unit,
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
                    name: "firmware",
                    duration: Duration::from_secs(26),
                },
                BootPhase {
                    name: "kernel",
                    duration: Duration::from_millis(500),
                },
                BootPhase {
                    name: "initrd",
                    duration: Duration::from_millis(3500),
                },
                BootPhase {
                    name: "userspace",
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
        let names: Vec<(&str, bool)> = slowest
            .slowest_first
            .iter()
            .map(|activation| (activation.name.as_str(), activation.slow))
            .collect();
        assert_eq!(
            names,
            [("systemd-rfkill.service", true), ("dbus.service", false)]
        );
    }
}
