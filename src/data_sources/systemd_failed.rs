//! Failed systemd units (system + user scope) as a bar indicator, ported from
//! odin's `systemd-failed.nu`.
//!
//! Polled over D-Bus (`org.freedesktop.systemd1`) with `zbus`'s blocking API,
//! so it fits the sync [`Source`] poll. The count is a badge; the tooltip is a
//! timestamped [`Log`](crate::data_sources::TooltipBody::Log) of the failed
//! units, newest first. Severity is horizon-gated: a unit only lights the bar
//! (warning/critical) if it entered its failed state within [`HORIZON`], so
//! stale failures don't keep it lit; a *recent system* failure is critical, all
//! others warning. An empty scope takes the widget off the bar.

use std::time::{Duration, SystemTime};

use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedObjectPath,
};

use crate::{
    data_sources::{BarWidget, Level, LogEntry, Source, TooltipBody},
    widgets::icon::Icon,
};

/// Only failures within this window colour the bar (odin's `horizon`).
const HORIZON: Duration = Duration::from_secs(3600);
const POLL: Duration = Duration::from_secs(30);

/// The `ListUnitsFiltered` row shape (a subset is used): name, description,
/// load, active, sub, following, unit path, job id, job type, job path.
type UnitRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);

#[derive(Debug, Clone)]
struct FailedUnit {
    name: String,
    reason: String,
    /// Wall-clock instant the unit entered its failed state.
    at: SystemTime,
    /// Whether that was within [`HORIZON`].
    recent: bool,
}

/// Failed units in one scope; `USER` selects the per-user manager (session bus)
/// over the system manager. Only constructed when at least one unit is failed.
#[derive(Debug, Clone)]
pub struct Failed<const USER: bool> {
    units: Vec<FailedUnit>,
}

impl<const USER: bool> Failed<USER> {
    /// A row's severity: a recent system-scope failure is critical (the most
    /// urgent), every other still-failed unit is at least warning.
    fn row_level(recent: bool) -> Level {
        if recent && !USER {
            Level::Critical
        } else {
            Level::Warning
        }
    }
}

impl<const USER: bool> BarWidget for Failed<USER> {
    fn icon(&self) -> Icon {
        if USER { Icon::User } else { Icon::Server }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.units.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        // Every failed unit is at least warning; critical only when a recent
        // system-scope failure is present (odin's most-severe-row rule).
        if !USER && self.units.iter().any(|unit| unit.recent) {
            Level::Critical
        } else {
            Level::Warning
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let mut units: Vec<&FailedUnit> = self.units.iter().collect();
        units.sort_by_key(|unit| std::cmp::Reverse(unit.at));
        TooltipBody::Log(
            units
                .into_iter()
                .map(|unit| LogEntry {
                    at: unit.at,
                    message: format!("{} ({})", unit.name, unit.reason),
                    level: Self::row_level(unit.recent),
                })
                .collect(),
        )
    }
}

impl Source for Failed<false> {
    const NAME: &'static str = "systemd-failed-system";
    const INTERVAL: Duration = POLL;

    fn poll() -> Option<Self> {
        poll_failed(false).map(|units| Self { units })
    }
}

impl Source for Failed<true> {
    const NAME: &'static str = "systemd-failed-user";
    const INTERVAL: Duration = POLL;

    fn poll() -> Option<Self> {
        poll_failed(true).map(|units| Self { units })
    }
}

fn poll_failed(user: bool) -> Option<Vec<FailedUnit>> {
    let connection = if user {
        Connection::session()
    } else {
        Connection::system()
    }
    .ok()?;
    let manager = Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )
    .ok()?;
    let rows: Vec<UnitRow> = manager.call("ListUnitsFiltered", &(vec!["failed"],)).ok()?;
    if rows.is_empty() {
        return None;
    }
    let uptime = uptime()?;
    let now = SystemTime::now();
    Some(
        rows.into_iter()
            .map(|row| failed_unit(&connection, row, uptime, now))
            .collect(),
    )
}

fn failed_unit(
    connection: &Connection,
    row: UnitRow,
    uptime: Duration,
    now: SystemTime,
) -> FailedUnit {
    let name = row.0;
    let (result, status, changed_us) = details(connection, &row.6);
    // systemd's monotonic timestamp is µs since boot; age it against the current
    // uptime and reproject onto the wall clock, exactly as odin does — avoids any
    // timezone parsing.
    let changed = Duration::from_micros(changed_us);
    let age = uptime.checked_sub(changed).unwrap_or_default();
    FailedUnit {
        name,
        reason: describe_failure(&result, status),
        at: now.checked_sub(age).unwrap_or(now),
        recent: changed_us > 0 && age <= HORIZON,
    }
}

/// Read a failed unit's `Result`, main exit status, and last state-change
/// monotonic timestamp. Non-service units (mounts, timers) have no `Service`
/// interface, so those default gracefully.
fn details(connection: &Connection, path: &OwnedObjectPath) -> (String, i32, u64) {
    let changed_us = property(
        connection,
        path,
        "org.freedesktop.systemd1.Unit",
        "StateChangeTimestampMonotonic",
    )
    .unwrap_or(0);
    let result = property(
        connection,
        path,
        "org.freedesktop.systemd1.Service",
        "Result",
    )
    .unwrap_or_else(|| "failed".to_string());
    let status = property(
        connection,
        path,
        "org.freedesktop.systemd1.Service",
        "ExecMainStatus",
    )
    .unwrap_or(0);
    (result, status, changed_us)
}

/// One D-Bus property off an interface of a unit's object, or `None` if absent
/// (the interface may not exist on this unit type, or the read may fail).
fn property<T>(
    connection: &Connection,
    path: &OwnedObjectPath,
    interface: &str,
    name: &str,
) -> Option<T>
where
    T: TryFrom<zbus::zvariant::OwnedValue>,
    <T as TryFrom<zbus::zvariant::OwnedValue>>::Error: Into<zbus::Error>,
{
    Proxy::new(
        connection,
        "org.freedesktop.systemd1",
        path.as_ref(),
        interface,
    )
    .ok()
    .and_then(|proxy| proxy.get_property(name).ok())
}

/// systemd's terse `Result` enum (+ main exit status) as a plain-language
/// phrase (odin's `describe-failure`).
fn describe_failure(result: &str, status: i32) -> String {
    match result {
        "exit-code" => format!("exited with code {status}"),
        "signal" => format!("killed by signal {status}"),
        "core-dump" => "crashed (core dumped)".to_string(),
        "timeout" => "timed out".to_string(),
        "watchdog" => "watchdog timeout".to_string(),
        "oom-kill" => "killed (out of memory)".to_string(),
        "start-limit-hit" => "too many restarts".to_string(),
        "resources" => "could not start (resources)".to_string(),
        other => other.to_string(),
    }
}

/// Seconds since boot from `/proc/uptime` — systemd's monotonic clock
/// reference.
fn uptime() -> Option<Duration> {
    let content = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = content.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_secs_f64(secs))
}
