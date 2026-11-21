use std::{cmp::Reverse, time::Duration};

use futures::{StreamExt, stream};
use sottomano_toml_config::severity::Severity;

use super::MonotonicStamp;
use crate::systemd_units_journal::{ManagerProxy, UnitRow, unit_proxy};

const SLOWEST_UNIT_COUNT: usize = 10;
const UNIT_STAMP_READS_IN_FLIGHT: usize = 16;
const SHORTEST_REPORTED_ACTIVATION: Duration = Duration::from_millis(100);
const DEVICE_UNIT_SUFFIX: &str = ".device";

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

struct ActivationStamps {
    name: String,
    inactive_exit: MonotonicStamp,
    active_enter: MonotonicStamp,
}

pub(super) async fn slowest_activations(
    manager: &ManagerProxy<'_>,
    finish: Duration,
    slow_unit: Duration,
) -> zbus::Result<RankedActivations> {
    let connection = manager.inner().connection();
    let rows = manager.list_units().await?;
    let stamps: Vec<_> = stream::iter(
        rows.into_iter()
            .filter(|row| !row.name.ends_with(DEVICE_UNIT_SUFFIX)),
    )
    .map(|row| activation_stamps(connection, row))
    .buffer_unordered(UNIT_STAMP_READS_IN_FLIGHT)
    .collect()
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
    let stamps_read = async {
        let unit = unit_proxy(connection, row.unit_path).await?;
        Ok::<_, zbus::Error>(ActivationStamps {
            inactive_exit: unit.inactive_exit_timestamp_monotonic().await?.into(),
            active_enter: unit.active_enter_timestamp_monotonic().await?.into(),
            name: row.name.clone(),
        })
    };
    stamps_read
        .await
        .inspect_err(|stamps_failure| {
            tracing::debug!(
                "systemd startup: activation stamps of {}: {stamps_failure}",
                row.name
            );
        })
        .ok()
}

fn slowest_before_finish(
    stamps: impl IntoIterator<Item = ActivationStamps>,
    finish: Duration,
    slow_unit: Duration,
) -> RankedActivations {
    let mut activations: Vec<UnitActivation> = stamps
        .into_iter()
        .filter_map(|stamps| {
            let MonotonicStamp::SinceBoot(active_enter) = stamps.active_enter else {
                return None;
            };
            let MonotonicStamp::SinceBoot(inactive_exit) = stamps.inactive_exit else {
                return None;
            };
            if active_enter > finish {
                return None;
            }
            let duration = active_enter
                .checked_sub(inactive_exit)
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

    #[test]
    fn blame_skips_instant_units_and_units_activated_after_boot() {
        let stamps = |name: &str, began: f64, entered: f64| ActivationStamps {
            name: name.to_owned(),
            inactive_exit: MonotonicStamp::SinceBoot(seconds(began)),
            active_enter: MonotonicStamp::SinceBoot(seconds(entered)),
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
