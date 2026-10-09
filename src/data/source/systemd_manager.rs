pub mod boot_timeline;
pub mod failed_units;
pub mod running_jobs;

use std::{
    future::pending,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iced::futures::{
    future::join_all,
    stream::{SelectAll, select_all},
};
use serde::Deserialize;
use tokio::time::sleep;
use zbus::{
    MatchRule, MessageStream,
    match_rule::Builder,
    message, proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

use crate::data::source::journal::either_of;

const SYSTEMD_SERVICE: &str = "org.freedesktop.systemd1";
const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";

pub trait UnitScope {
    const PER_USER: bool;
    const CLI_FLAG: Option<&'static str>;
    const LOGGED_BY: &'static str;
    const LOGGED_ABOUT: &'static str;

    fn follow_units_log_args<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        Self::CLI_FLAG
            .map(String::from)
            .into_iter()
            .chain(["-x", "-b", "-n", "all", "-f"].map(String::from))
            .chain(either_of(names.into_iter().flat_map(Self::unit_matches)))
            .collect()
    }

    fn unit_matches(name: &str) -> [String; 4] {
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        [
            format!("{}={name}", Self::LOGGED_BY),
            format!("{}={name}", Self::LOGGED_ABOUT),
            format!("SYSLOG_IDENTIFIER={stem}"),
            format!("SYSLOG_IDENTIFIER={stem}-start"),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User;

impl UnitScope for User {
    const PER_USER: bool = true;
    const CLI_FLAG: Option<&'static str> = Some("--user");
    const LOGGED_BY: &'static str = "_SYSTEMD_USER_UNIT";
    const LOGGED_ABOUT: &'static str = "USER_UNIT";
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System;

impl UnitScope for System {
    const PER_USER: bool = false;
    const CLI_FLAG: Option<&'static str> = None;
    const LOGGED_BY: &'static str = "_SYSTEMD_UNIT";
    const LOGGED_ABOUT: &'static str = "UNIT";
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1",
    gen_blocking = false
)]
trait Manager {
    fn list_units(&self) -> zbus::Result<Vec<UnitRow>>;
    fn list_units_filtered(&self, states: &[&str]) -> zbus::Result<Vec<UnitRow>>;
    fn list_jobs(&self) -> zbus::Result<Vec<JobRow>>;
    fn subscribe(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn firmware_timestamp_monotonic(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn loader_timestamp_monotonic(&self) -> zbus::Result<u64>;
    #[zbus(property, name = "InitRDTimestampMonotonic")]
    fn initrd_timestamp_monotonic(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn userspace_timestamp_monotonic(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn finish_timestamp_monotonic(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Unit {
    #[zbus(property)]
    fn state_change_timestamp(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn inactive_exit_timestamp_monotonic(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn active_enter_timestamp_monotonic(&self) -> zbus::Result<u64>;
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

#[derive(Debug, Deserialize, Type)]
#[allow(dead_code)]
struct JobRow {
    id: u32,
    unit: String,
    job_type: String,
    state: String,
    job_path: OwnedObjectPath,
    unit_path: OwnedObjectPath,
}

async fn connect(user: bool) -> zbus::Result<zbus::Connection> {
    if user {
        zbus::Connection::session().await
    } else {
        zbus::Connection::system().await
    }
}

fn systemd_signal() -> zbus::Result<Builder<'static>> {
    MatchRule::builder()
        .msg_type(message::Type::Signal)
        .sender(SYSTEMD_SERVICE)
}

fn manager_signal(member: &'static str) -> zbus::Result<MatchRule<'static>> {
    Ok(systemd_signal()?
        .interface(MANAGER_INTERFACE)?
        .member(member)?
        .build())
}

async fn signals_matching(
    connection: &zbus::Connection,
    rules: impl IntoIterator<Item = MatchRule<'static>>,
) -> zbus::Result<SelectAll<MessageStream>> {
    let streams = join_all(
        rules
            .into_iter()
            .map(|rule| MessageStream::for_match_rule(rule, connection, Some(64))),
    )
    .await
    .into_iter()
    .collect::<zbus::Result<Vec<_>>>()?;
    Ok(select_all(streams))
}

fn wall_time_of_realtime_usec(realtime_us: u64) -> Option<SystemTime> {
    (realtime_us > 0).then(|| UNIX_EPOCH + Duration::from_micros(realtime_us))
}

async fn unit_proxy(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
) -> zbus::Result<UnitProxy<'static>> {
    UnitProxy::builder(connection)
        .path(path)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn state_changed_at(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
) -> Option<SystemTime> {
    let unit = unit_proxy(connection, path).await.ok()?;
    wall_time_of_realtime_usec(unit.state_change_timestamp().await.ok()?)
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
