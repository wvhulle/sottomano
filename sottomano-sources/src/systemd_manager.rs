pub mod boot_timeline;
pub mod failed_units;
pub mod graphical_session;
pub mod running_jobs;
pub mod service_commands;
pub mod unit_activity;
pub mod user_service_manager;

use std::{
    hash::Hash,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::{
    future::join_all,
    stream::{SelectAll, select_all},
};
use serde::Deserialize;
use zbus::{
    MatchRule, MessageStream,
    match_rule::Builder,
    message, proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

use crate::{
    bus::{Bus, DesktopBuses},
    journal::either_of,
};

const SYSTEMD_SERVICE: &str = "org.freedesktop.systemd1";
const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedSystemdUnit {
    pub scope: SystemdUnitScope,
    pub unit_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemdOnBus {
    pub buses: DesktopBuses,
    pub scope: SystemdUnitScope,
}

impl SystemdOnBus {
    pub fn connection(&self) -> zbus::Connection {
        self.buses.on(self.scope.bus()).clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemdUnitOnBus {
    pub buses: DesktopBuses,
    pub unit: ScopedSystemdUnit,
}

impl SystemdUnitOnBus {
    pub fn connection(&self) -> zbus::Connection {
        self.buses.on(self.unit.scope.bus()).clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemdUnitScope {
    UserSession,
    SystemWide,
}

impl SystemdUnitScope {
    #[must_use]
    pub const fn per_user(self) -> bool {
        match self {
            SystemdUnitScope::UserSession => true,
            SystemdUnitScope::SystemWide => false,
        }
    }

    #[must_use]
    pub const fn bus(self) -> Bus {
        match self {
            SystemdUnitScope::UserSession => Bus::Session,
            SystemdUnitScope::SystemWide => Bus::System,
        }
    }

    #[must_use]
    pub const fn journal_logged_by_field(self) -> &'static str {
        match self {
            SystemdUnitScope::UserSession => "_SYSTEMD_USER_UNIT",
            SystemdUnitScope::SystemWide => "_SYSTEMD_UNIT",
        }
    }

    const fn journal_logged_about_field(self) -> &'static str {
        match self {
            SystemdUnitScope::UserSession => "USER_UNIT",
            SystemdUnitScope::SystemWide => "UNIT",
        }
    }

    const fn journalctl_scope_flag(self) -> Option<&'static str> {
        match self {
            SystemdUnitScope::UserSession => Some("--user"),
            SystemdUnitScope::SystemWide => None,
        }
    }

    pub fn follow_units_log_args<'a>(
        self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Vec<String> {
        self.journalctl_scope_flag()
            .map(String::from)
            .into_iter()
            .chain(["-x", "-b", "-n", "all", "-f"].map(String::from))
            .chain(either_of(
                names.into_iter().flat_map(|name| self.unit_matches(name)),
            ))
            .collect()
    }

    fn unit_matches(self, name: &str) -> [String; 4] {
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        [
            format!("{}={name}", self.journal_logged_by_field()),
            format!("{}={name}", self.journal_logged_about_field()),
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
    fn list_units(&self) -> zbus::Result<Vec<UnitRow>>;
    fn list_units_filtered(&self, states: &[&str]) -> zbus::Result<Vec<UnitRow>>;
    fn list_jobs(&self) -> zbus::Result<Vec<JobRow>>;
    fn subscribe(&self) -> zbus::Result<()>;
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;
    fn get_unit(&self, name: &str) -> zbus::Result<OwnedObjectPath>;
    fn load_unit(&self, name: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(signal)]
    fn job_removed(
        &self,
        id: u32,
        job: OwnedObjectPath,
        unit: String,
        result: String,
    ) -> zbus::Result<()>;
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
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn system_state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn userspace_timestamp(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Unit {
    #[zbus(property)]
    fn active_state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn active_enter_timestamp(&self) -> zbus::Result<u64>;
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

const ALREADY_SUBSCRIBED: &str = "org.freedesktop.systemd1.AlreadySubscribed";

impl ManagerProxy<'_> {
    async fn ensure_subscribed(&self) -> zbus::Result<()> {
        match self.subscribe().await {
            Err(zbus::Error::MethodError(name, _, _)) if name == ALREADY_SUBSCRIBED => Ok(()),
            outcome => outcome,
        }
    }
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
