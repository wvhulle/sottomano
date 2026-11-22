use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use procfs::process::Process as Proc;
use zbus::{proxy, proxy::CacheProperties, zvariant::OwnedObjectPath};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    host_machine_identity::hostname1_os_identity::at_micros,
    login_session_presence::{
        greetd_greeter_program::greeter_program,
        logind_manager::{ManagerProxy, session_id},
    },
    non_empty_text::non_empty,
    system_protocol_clients::{
        dbus_change_signals::{change_triggers, properties_changed_rule},
        dbus_desktop_buses::{BusReadError, DesktopBuses},
    },
    systemd_units_journal::graphical_session_target::DesktopName,
};

const LOGIN_SERVICE: &str = "org.freedesktop.login1";
const SESSION_INTERFACE: &str = "org.freedesktop.login1.Session";

#[proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1",
    gen_blocking = false
)]
trait Session {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn user(&self) -> zbus::Result<(u32, OwnedObjectPath)>;
    #[zbus(property)]
    fn seat(&self) -> zbus::Result<(String, OwnedObjectPath)>;
    #[zbus(property, name = "VTNr")]
    fn vt_nr(&self) -> zbus::Result<u32>;
    #[zbus(property, name = "Type")]
    fn session_type(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn class(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn desktop(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn service(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn scope(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn leader(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn timestamp(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn idle_hint(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn idle_since_hint(&self) -> zbus::Result<u64>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdentity {
    pub user: String,
    pub uid: u32,
    pub seat: String,
    pub virtual_terminal: VirtualTerminal,
    pub login_manager: String,
    pub desktop: DesktopName,
    pub session_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualTerminal {
    Console(NonZeroU32),
    NoConsole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaderName {
    Named(String),
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreeterProgram {
    Identified(String),
    Unidentified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIdleness {
    IdleSince(DateTime<Utc>),
    Active,
}

impl SnapshotSource for SessionIdentity {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Session identity";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, identity_changes)
            .map(SnapshotDelivery::unavailable_when_absent)
    }
}

async fn identity_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<SessionIdentity, BusReadError>> + Send, BusReadError> {
    let session = own_session(&buses.system).await?;
    let identity = SessionIdentity {
        user: non_empty(session.name().await?).ok_or_else(|| {
            BusReadError::UnexpectedReply("logind session has no user name".to_owned())
        })?,
        uid: session.user().await?.0,
        seat: non_empty(session.seat().await?.0).ok_or_else(|| {
            BusReadError::UnexpectedReply("logind session has no seat".to_owned())
        })?,
        virtual_terminal: match NonZeroU32::new(session.vt_nr().await?) {
            Some(console) => VirtualTerminal::Console(console),
            None => VirtualTerminal::NoConsole,
        },
        login_manager: session.service().await?,
        desktop: match session.desktop().await {
            Ok(desktop_name) if !desktop_name.is_empty() => DesktopName::Named(desktop_name),
            Ok(_) => DesktopName::Unnamed,
            Err(desktop_failure) => {
                tracing::warn!("logind session desktop: {desktop_failure}");
                DesktopName::Unnamed
            }
        },
        session_type: String::new(),
    };
    let changes = change_triggers(
        &buses.system,
        [properties_changed_rule(LOGIN_SERVICE, SESSION_INTERFACE)?],
    )
    .await?;
    Ok(changes.triggered_rereads(move || {
        let session = session.clone();
        let identity = identity.clone();
        async move {
            Ok(SessionIdentity {
                session_type: session.session_type().await?,
                ..identity
            })
        }
    }))
}

async fn own_session(system: &zbus::Connection) -> Result<SessionProxy<'static>, BusReadError> {
    let manager = ManagerProxy::builder(system)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let path = manager.get_session(&session_id()).await?;
    let session_builder = SessionProxy::builder(system)
        .cache_properties(CacheProperties::No)
        .path(path)?;
    Ok(session_builder.build().await?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub scope: String,
    pub leader: u32,
    pub leader_name: LeaderName,
    pub greeter: GreeterProgram,
    pub state: String,
    pub since: DateTime<Utc>,
    pub idleness: SessionIdleness,
}

impl SnapshotSource for SessionInfo {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Login session";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, session_changes)
            .map(SnapshotDelivery::unavailable_when_absent)
    }
}

async fn session_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<SessionInfo, BusReadError>> + Send, BusReadError> {
    let session = own_session(&buses.system).await?;
    let leader = session.leader().await?;
    let info = SessionInfo {
        id: session.id().await?,
        scope: session.scope().await?,
        leader,
        leader_name: process_name(leader).map_or(LeaderName::Unreadable, LeaderName::Named),
        greeter: greeter_program(leader)
            .map_or(GreeterProgram::Unidentified, GreeterProgram::Identified),
        state: session.state().await?,
        since: at_micros(session.timestamp().await?).ok_or_else(|| {
            BusReadError::UnexpectedReply("logind session start time is out of range".to_owned())
        })?,
        idleness: SessionIdleness::Active,
    };
    let changes = change_triggers(
        &buses.system,
        [properties_changed_rule(LOGIN_SERVICE, SESSION_INTERFACE)?],
    )
    .await?;
    Ok(changes.triggered_rereads(move || {
        let session = session.clone();
        let info = info.clone();
        async move {
            let idleness = if session.idle_hint().await? {
                at_micros(session.idle_since_hint().await?)
                    .map_or(SessionIdleness::Active, SessionIdleness::IdleSince)
            } else {
                SessionIdleness::Active
            };
            Ok(SessionInfo {
                state: session.state().await?,
                idleness,
                ..info
            })
        }
    }))
}

fn process_name(pid: u32) -> Option<String> {
    let process_id = i32::try_from(pid)
        .inspect_err(|range_failure| {
            tracing::warn!("logind session leader pid {pid} out of range: {range_failure}");
        })
        .ok()?;
    let process = Proc::new(process_id)
        .inspect_err(|proc_failure| {
            tracing::debug!("logind session leader {pid} unreadable: {proc_failure}");
        })
        .ok()?;
    let process_stat = process
        .stat()
        .inspect_err(|stat_failure| {
            tracing::debug!("logind session leader {pid} stat unreadable: {stat_failure}");
        })
        .ok()?;
    non_empty(process_stat.comm)
}
