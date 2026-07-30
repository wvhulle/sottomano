//! Failed systemd units (system + user scope) as a bar indicator, ported from
//! odin's `systemd-failed.nu`.
//!
//! Polled over D-Bus (`org.freedesktop.systemd1`) with `zbus`'s blocking API,
//! so it fits the sync [`Source`] poll. The count is a badge; the tooltip is a
//! timestamped [`Log`](crate::data::source::TooltipBody::Log) of the failed
//! units, newest first. Severity is horizon-gated: a unit only lights the bar
//! (warning/critical) if it entered its failed state within [`HORIZON`], so
//! stale failures don't keep it lit; a *recent system* failure is critical, all
//! others warning. An empty scope takes the widget off the bar.

use std::time::{Duration, SystemTime};

use serde::Deserialize;
use zbus::{
    blocking::Connection,
    proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait Manager {
    fn list_units_filtered(&self, states: &[&str]) -> zbus::Result<Vec<UnitRow>>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait Unit {
    #[zbus(property)]
    fn state_change_timestamp_monotonic(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Service",
    default_service = "org.freedesktop.systemd1"
)]
trait Service {
    #[zbus(property)]
    fn result(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn exec_main_status(&self) -> zbus::Result<i32>;
}

use crate::data::source::{
    BarWidget, Glyph, Level, LogEntry, Source, TooltipBody, font_awesome::Solid,
};

/// Only failures within this window colour the bar (odin's `horizon`).
const HORIZON: Duration = Duration::from_secs(3600);
const POLL: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize, Type)]
#[allow(dead_code)]
struct UnitRow {
    name: String,
    description: String,
    load_state: String,
    active_state: String,
    sub_state: String,
    following: String,
    unit_path: OwnedObjectPath,
    job_id: u32,
    job_type: String,
    job_path: OwnedObjectPath,
}

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
    fn icon(&self) -> Glyph {
        if USER {
            Solid::User.into()
        } else {
            Solid::Server.into()
        }
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
    let manager = ManagerProxyBlocking::new(&connection).ok()?;
    let rows = manager.list_units_filtered(&["failed"]).ok()?;
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
    let name = row.name;
    let UnitDetails {
        result,
        status,
        changed_us,
    } = details(connection, &row.unit_path);
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
struct UnitDetails {
    result: String,
    status: i32,
    changed_us: u64,
}

fn details(connection: &Connection, path: &OwnedObjectPath) -> UnitDetails {
    let unit = UnitProxyBlocking::builder(connection)
        .path(path.clone())
        .ok()
        .and_then(|builder| builder.cache_properties(CacheProperties::No).build().ok());
    let service = ServiceProxyBlocking::builder(connection)
        .path(path.clone())
        .ok()
        .and_then(|builder| builder.cache_properties(CacheProperties::No).build().ok());
    let changed_us = unit
        .and_then(|unit| unit.state_change_timestamp_monotonic().ok())
        .unwrap_or(0);
    let result = service
        .as_ref()
        .and_then(|service| service.result().ok())
        .unwrap_or_else(|| "failed".to_string());
    let status = service
        .and_then(|service| service.exec_main_status().ok())
        .unwrap_or(0);
    UnitDetails {
        result,
        status,
        changed_us,
    }
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
