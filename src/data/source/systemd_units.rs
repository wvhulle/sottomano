use std::{
    fs,
    future::pending,
    marker::PhantomData,
    pin::Pin,
    slice,
    time::{Duration, SystemTime},
};

use iced::futures::{
    Stream, StreamExt,
    future::join_all,
    stream::{self, select_all},
};
use serde::Deserialize;
use tokio::time::sleep;
use zbus::{
    MatchRule, MessageStream, message, proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

use crate::message_subscriptions::restarting;

pub const RECENT_FAILURE_HORIZON: Duration = Duration::from_secs(3600);

const SYSTEMD_SERVICE: &str = "org.freedesktop.systemd1";
const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";

pub trait UnitScope {
    const PER_USER: bool;
    const CLI_FLAG: Option<&'static str>;
    const LOGGED_BY: &'static str;
    const LOGGED_ABOUT: &'static str;
}

#[derive(Debug, Clone)]
pub struct User;

impl UnitScope for User {
    const PER_USER: bool = true;
    const CLI_FLAG: Option<&'static str> = Some("--user");
    const LOGGED_BY: &'static str = "_SYSTEMD_USER_UNIT";
    const LOGGED_ABOUT: &'static str = "USER_UNIT";
}

#[derive(Debug, Clone)]
pub struct System;

impl UnitScope for System {
    const PER_USER: bool = false;
    const CLI_FLAG: Option<&'static str> = None;
    const LOGGED_BY: &'static str = "_SYSTEMD_UNIT";
    const LOGGED_ABOUT: &'static str = "UNIT";
}

#[derive(Debug, Clone)]
pub struct ServiceOutcome {
    pub result: String,
    pub exit_status: i32,
}

#[derive(Debug, Clone)]
pub struct FailedUnitInfo {
    pub name: String,
    pub service_outcome: Option<ServiceOutcome>,
    pub at: Option<SystemTime>,
}

impl FailedUnitInfo {
    pub fn recent(&self) -> bool {
        self.at
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age <= RECENT_FAILURE_HORIZON)
    }

    fn recency_expiry(&self) -> Option<SystemTime> {
        Some(self.at? + RECENT_FAILURE_HORIZON).filter(|expiry| *expiry > SystemTime::now())
    }
}

#[derive(Debug, Clone)]
pub struct Failed<S: UnitScope> {
    pub units: Vec<FailedUnitInfo>,
    scope: PhantomData<S>,
}

impl<S: UnitScope> Failed<S> {
    pub fn from_units(units: Vec<FailedUnitInfo>) -> Option<Self> {
        (!units.is_empty()).then_some(Self {
            units,
            scope: PhantomData,
        })
    }

    pub fn follow_log_args(&self) -> Vec<String> {
        Self::follow_log_args_for(&self.units)
    }

    pub fn follow_unit_log_args(unit: &FailedUnitInfo) -> Vec<String> {
        Self::follow_log_args_for(slice::from_ref(unit))
    }

    fn follow_log_args_for(units: &[FailedUnitInfo]) -> Vec<String> {
        S::CLI_FLAG
            .map(String::from)
            .into_iter()
            .chain(["-x", "-b", "-n", "all", "-f"].map(String::from))
            .chain(Self::journal_matches(units))
            .collect()
    }

    fn journal_matches(units: &[FailedUnitInfo]) -> impl Iterator<Item = String> {
        units
            .iter()
            .flat_map(|unit| Self::matches_for(&unit.name))
            .enumerate()
            .flat_map(|(index, matcher)| {
                (index > 0)
                    .then(|| String::from("+"))
                    .into_iter()
                    .chain([matcher])
            })
    }

    fn matches_for(name: &str) -> [String; 4] {
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        [
            format!("{}={name}", S::LOGGED_BY),
            format!("{}={name}", S::LOGGED_ABOUT),
            format!("SYSLOG_IDENTIFIER={stem}"),
            format!("SYSLOG_IDENTIFIER={stem}-start"),
        ]
    }
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1",
    gen_blocking = false
)]
trait Manager {
    fn list_units_filtered(&self, states: &[&str]) -> zbus::Result<Vec<UnitRow>>;
    fn subscribe(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Unit {
    #[zbus(property)]
    fn state_change_timestamp_monotonic(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Service",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Service {
    #[zbus(property)]
    fn result(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn exec_main_status(&self) -> zbus::Result<i32>;
}

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

pub fn failed_stream(per_user: bool) -> impl Stream<Item = Vec<FailedUnitInfo>> {
    restarting("systemd units", move || watch_failed(per_user))
}

async fn watch_failed(user: bool) -> zbus::Result<impl Stream<Item = Vec<FailedUnitInfo>>> {
    let connection = connect(user).await?;
    let manager = ManagerProxy::new(&connection).await?;
    if let Err(e) = manager.subscribe().await {
        tracing::warn!("systemd units: subscribe failed: {e}");
    }
    let unit_changes = unit_state_changes(&connection).await?;
    Ok(stream::unfold(
        FailedUnitWatch {
            manager,
            unit_changes: Box::pin(unit_changes),
            last_report: None,
        },
        FailedUnitWatch::next_report,
    ))
}

struct FailedUnitWatch {
    manager: ManagerProxy<'static>,
    unit_changes: Pin<Box<dyn Stream<Item = ()> + Send>>,
    last_report: Option<Vec<FailedUnitInfo>>,
}

impl FailedUnitWatch {
    async fn next_report(mut self) -> Option<(Vec<FailedUnitInfo>, Self)> {
        let units = match self.last_report.take() {
            None => failed_units(&self.manager).await,
            Some(last_report) => {
                let recency_expiry = last_report
                    .iter()
                    .filter_map(FailedUnitInfo::recency_expiry)
                    .min();
                tokio::select! {
                    change = self.unit_changes.next() => {
                        change?;
                        failed_units(&self.manager).await
                    }
                    () = sleep_until(recency_expiry) => last_report,
                }
            }
        };
        self.last_report = Some(units.clone());
        Some((units, self))
    }
}

async fn sleep_until(deadline: Option<SystemTime>) {
    match deadline {
        Some(deadline) => {
            let remaining = deadline
                .duration_since(SystemTime::now())
                .unwrap_or(Duration::ZERO);
            sleep(remaining).await;
        }
        None => pending().await,
    }
}

async fn connect(user: bool) -> zbus::Result<zbus::Connection> {
    if user {
        zbus::Connection::session().await
    } else {
        zbus::Connection::system().await
    }
}

async fn unit_state_changes(
    connection: &zbus::Connection,
) -> zbus::Result<impl Stream<Item = ()> + Send + use<>> {
    let systemd_signal = || {
        MatchRule::builder()
            .msg_type(message::Type::Signal)
            .sender(SYSTEMD_SERVICE)
    };
    let rules = [
        systemd_signal()?
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .arg(0, UNIT_INTERFACE)?
            .build(),
        systemd_signal()?
            .interface(MANAGER_INTERFACE)?
            .member("UnitRemoved")?
            .build(),
        systemd_signal()?
            .interface(MANAGER_INTERFACE)?
            .member("Reloading")?
            .build(),
    ];
    let streams = join_all(
        rules
            .into_iter()
            .map(|rule| MessageStream::for_match_rule(rule, connection, Some(64))),
    )
    .await
    .into_iter()
    .collect::<zbus::Result<Vec<_>>>()?;
    Ok(select_all(streams).ready_chunks(64).map(|_| ()))
}

async fn failed_units(manager: &ManagerProxy<'static>) -> Vec<FailedUnitInfo> {
    let Ok(rows) = manager.list_units_filtered(&["failed"]).await else {
        return Vec::new();
    };
    let uptime = uptime();
    let now = SystemTime::now();
    let connection = manager.inner().connection();
    join_all(
        rows.into_iter()
            .map(|row| failed_unit(connection, row, uptime, now)),
    )
    .await
}

async fn failed_unit(
    connection: &zbus::Connection,
    row: UnitRow,
    uptime: Option<Duration>,
    now: SystemTime,
) -> FailedUnitInfo {
    let changed_us = state_change_monotonic_us(connection, row.unit_path.clone()).await;
    FailedUnitInfo {
        name: row.name,
        service_outcome: service_outcome(connection, row.unit_path).await,
        at: changed_us
            .zip(uptime)
            .and_then(|(changed_us, uptime)| failed_at(changed_us, uptime, now)),
    }
}

fn failed_at(changed_us: u64, uptime: Duration, now: SystemTime) -> Option<SystemTime> {
    let age = uptime.checked_sub(Duration::from_micros(changed_us))?;
    now.checked_sub(age)
}

async fn state_change_monotonic_us(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
) -> Option<u64> {
    let unit = UnitProxy::builder(connection)
        .path(path)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()?;
    unit.state_change_timestamp_monotonic()
        .await
        .ok()
        .filter(|changed_us| *changed_us > 0)
}

async fn service_outcome(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
) -> Option<ServiceOutcome> {
    let service = ServiceProxy::builder(connection)
        .path(path)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()?;
    Some(ServiceOutcome {
        result: service.result().await.ok()?,
        exit_status: service.exec_main_status().await.ok()?,
    })
}

fn uptime() -> Option<Duration> {
    let content = fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = content.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_secs_f64(secs))
}
