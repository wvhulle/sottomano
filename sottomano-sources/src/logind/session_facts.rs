use std::{num::NonZeroU32, str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use futures::{
    Stream, StreamExt,
    stream::{BoxStream, select_all},
};
use procfs::process::Process as Proc;
use zbus::{proxy, proxy::CacheProperties, zvariant::OwnedObjectPath};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    bus::{self, DesktopBuses},
    empty_text::non_empty,
    hostname::at_micros,
    logind::{InhibitorRow, ManagerProxy, greetd_greeter::greeter_program, session_id},
    pushed_delivery_stream,
};

#[proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
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
    pub virtual_terminal: Option<NonZeroU32>,
    pub login_manager: String,
    pub desktop: Option<String>,
    pub session_type: String,
}

impl ReadingSource for SessionIdentity {
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "logind session identity";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(identity_changes(buses))
    }
}

async fn identity_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<SessionIdentity, bus::Error>> + Send, bus::Error> {
    let session = own_session(&buses.system).await?;
    let identity = SessionIdentity {
        user: non_empty(session.name().await?).ok_or(bus::Error::Absent)?,
        uid: session.user().await?.0,
        seat: non_empty(session.seat().await?.0).ok_or(bus::Error::Absent)?,
        virtual_terminal: NonZeroU32::new(session.vt_nr().await?),
        login_manager: session.service().await?,
        desktop: session.desktop().await.ok().and_then(non_empty),
        session_type: session.session_type().await?,
    };
    let changes = session.receive_session_type_changed().await;
    Ok(changes.then(move |session_type| {
        let identity = identity.clone();
        async move {
            Ok(SessionIdentity {
                session_type: session_type.get().await?,
                ..identity
            })
        }
    }))
}

async fn own_session(system: &zbus::Connection) -> Result<SessionProxy<'static>, bus::Error> {
    let manager = ManagerProxy::new(system).await?;
    let path = manager.get_session(&session_id()).await?;
    let session_builder = SessionProxy::builder(system).path(path)?;
    Ok(session_builder.build().await?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub scope: String,
    pub leader: u32,
    pub leader_name: Option<String>,
    pub greeter: Option<String>,
    pub state: String,
    pub since: DateTime<Utc>,
    pub idle_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerPolicy {
    pub power_key: String,
    pub suspend_key: String,
    pub hibernate_key: String,
    pub reboot_key: String,
    pub lid_switch: String,
    pub chassis: Chassis,
    pub idle_action: String,
    pub idle_action_after: Option<Duration>,
    pub sleep_order: Vec<String>,
    pub inhibitor_locks: Vec<InhibitorLock>,
    pub sessions: u64,
    pub scheduled_shutdown: Option<ScheduledShutdown>,
    pub preparing_for_shutdown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InhibitMode {
    Block,
    Delay,
}

impl FromStr for InhibitMode {
    type Err = String;

    fn from_str(mode: &str) -> Result<Self, String> {
        match mode {
            "block" | "block-weak" => Ok(Self::Block),
            "delay" => Ok(Self::Delay),
            unknown => Err(format!("unknown inhibit mode {unknown}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InhibitorLock {
    pub who: String,
    pub operation: String,
    pub why: Option<String>,
    pub mode: InhibitMode,
}

impl InhibitorLock {
    fn per_operation(row: InhibitorRow) -> Vec<Self> {
        let Ok(mode) = row.mode.parse() else {
            return Vec::new();
        };
        let why = non_empty(row.why);
        row.what
            .split(':')
            .map(|operation| Self {
                who: row.who.clone(),
                operation: operation.to_owned(),
                why: why.clone(),
                mode,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chassis {
    pub docked: bool,
    pub lid_closed: bool,
    pub on_external_power: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledShutdown {
    pub kind: String,
    pub at: Option<DateTime<Utc>>,
}

impl ReadingSource for SessionInfo {
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "logind session";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(session_changes(buses))
    }
}

async fn session_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<SessionInfo, bus::Error>> + Send, bus::Error> {
    let session = own_session(&buses.system).await?;
    let leader = session.leader().await?;
    let info = SessionInfo {
        id: session.id().await?,
        scope: session.scope().await?,
        leader,
        leader_name: process_name(leader),
        greeter: greeter_program(leader),
        state: session.state().await?,
        since: at_micros(session.timestamp().await?).ok_or(bus::Error::Absent)?,
        idle_since: None,
    };
    let changes = select_all([
        session.receive_state_changed().await.map(drop).boxed(),
        session.receive_idle_hint_changed().await.map(drop).boxed(),
        session
            .receive_idle_since_hint_changed()
            .await
            .map(drop)
            .boxed(),
    ]);
    Ok(changes.then(move |()| {
        let session = session.clone();
        let info = info.clone();
        async move {
            let idle = session.idle_hint().await?;
            Ok(SessionInfo {
                state: session.state().await?,
                idle_since: idle
                    .then_some(session.idle_since_hint().await?)
                    .and_then(at_micros),
                ..info
            })
        }
    }))
}

impl ReadingSource for PowerPolicy {
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "logind power policy";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(power_changes(buses))
    }
}

async fn power_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<PowerPolicy, bus::Error>> + Send, bus::Error> {
    let signalled = ManagerProxy::new(&buses.system).await?;
    let uncached = ManagerProxy::builder(&buses.system)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let configured = ConfiguredPowerPolicy {
        power_key: uncached.handle_power_key().await?,
        suspend_key: uncached.handle_suspend_key().await?,
        hibernate_key: uncached.handle_hibernate_key().await?,
        reboot_key: uncached.handle_reboot_key().await?,
        lid_switch: uncached.handle_lid_switch().await?,
        idle_action: uncached.idle_action().await?,
        idle_action_after: Some(uncached.idle_action_usec().await?)
            .filter(|usec| *usec > 0 && *usec < u64::MAX)
            .map(Duration::from_micros),
        sleep_order: uncached.sleep_operation().await?,
    };
    let changes = select_all(power_triggers(&signalled).await?);
    Ok(changes.then(move |()| {
        let manager = uncached.clone();
        let configured = configured.clone();
        async move { read_power(&manager, configured).await }
    }))
}

#[derive(Clone)]
struct ConfiguredPowerPolicy {
    power_key: String,
    suspend_key: String,
    hibernate_key: String,
    reboot_key: String,
    lid_switch: String,
    idle_action: String,
    idle_action_after: Option<Duration>,
    sleep_order: Vec<String>,
}

async fn power_triggers(
    manager: &ManagerProxy<'static>,
) -> zbus::Result<[BoxStream<'static, ()>; 7]> {
    Ok([
        manager.receive_lid_closed_changed().await.map(drop).boxed(),
        manager
            .receive_scheduled_shutdown_changed()
            .await
            .map(drop)
            .boxed(),
        manager
            .receive_block_inhibited_changed()
            .await
            .map(drop)
            .boxed(),
        manager
            .receive_delay_inhibited_changed()
            .await
            .map(drop)
            .boxed(),
        manager
            .receive_prepare_for_shutdown()
            .await?
            .map(drop)
            .boxed(),
        manager.receive_session_new().await?.map(drop).boxed(),
        manager.receive_session_removed().await?.map(drop).boxed(),
    ])
}

fn process_name(pid: u32) -> Option<String> {
    let process = Proc::new(i32::try_from(pid).ok()?).ok()?;
    non_empty(process.stat().ok()?.comm)
}

async fn read_power(
    manager: &ManagerProxy<'_>,
    configured: ConfiguredPowerPolicy,
) -> Result<PowerPolicy, bus::Error> {
    let (kind, usec) = manager.scheduled_shutdown().await?;
    Ok(PowerPolicy {
        power_key: configured.power_key,
        suspend_key: configured.suspend_key,
        hibernate_key: configured.hibernate_key,
        reboot_key: configured.reboot_key,
        lid_switch: configured.lid_switch,
        idle_action: configured.idle_action,
        idle_action_after: configured.idle_action_after,
        sleep_order: configured.sleep_order,
        chassis: Chassis {
            docked: manager.docked().await?,
            lid_closed: manager.lid_closed().await?,
            on_external_power: manager.on_external_power().await?,
        },
        inhibitor_locks: manager
            .list_inhibitors()
            .await?
            .into_iter()
            .flat_map(InhibitorLock::per_operation)
            .collect(),
        sessions: manager.n_current_sessions().await?,
        scheduled_shutdown: non_empty(kind).map(|kind| ScheduledShutdown {
            kind,
            at: at_micros(usec),
        }),
        preparing_for_shutdown: manager.preparing_for_shutdown().await?,
    })
}
