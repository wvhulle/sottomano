use std::{num::NonZeroU32, str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use iced::futures::Stream;
use procfs::process::Process as Proc;
use zbus::{proxy, zvariant::OwnedObjectPath};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    bus::{self, DesktopBuses},
    empty_text::non_empty,
    hostname::at_micros,
    logind::{InhibitorRow, ManagerProxy, greetd_greeter::greeter_program, session_id},
    polled_delivery_stream,
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
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_hours(1));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl SessionIdentity {
    async fn poll(buses: DesktopBuses) -> Result<Self, bus::Error> {
        let session = own_session(&buses.system).await?;
        Ok(SessionIdentity {
            user: non_empty(session.name().await?).ok_or(bus::Error::Absent)?,
            uid: session.user().await?.0,
            seat: non_empty(session.seat().await?.0).ok_or(bus::Error::Absent)?,
            virtual_terminal: NonZeroU32::new(session.vt_nr().await?),
            login_manager: session.service().await?,
            desktop: session.desktop().await.ok().and_then(non_empty),
            session_type: session.session_type().await?,
        })
    }
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
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl SessionInfo {
    async fn poll(buses: DesktopBuses) -> Result<Self, bus::Error> {
        read_session(&own_session(&buses.system).await?).await
    }
}

impl ReadingSource for PowerPolicy {
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "logind power policy";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl PowerPolicy {
    async fn poll(buses: DesktopBuses) -> Result<Self, bus::Error> {
        let manager = ManagerProxy::new(&buses.system).await?;
        read_power(&manager).await
    }
}

fn process_name(pid: u32) -> Option<String> {
    let process = Proc::new(i32::try_from(pid).ok()?).ok()?;
    non_empty(process.stat().ok()?.comm)
}

async fn read_session(session: &SessionProxy<'_>) -> Result<SessionInfo, bus::Error> {
    let idle = session.idle_hint().await?;
    let since = session.timestamp().await?;
    let leader = session.leader().await?;
    Ok(SessionInfo {
        id: session.id().await?,
        scope: session.scope().await?,
        leader,
        leader_name: process_name(leader),
        greeter: greeter_program(leader),
        state: session.state().await?,
        since: at_micros(since).ok_or(bus::Error::Absent)?,
        idle_since: idle
            .then_some(session.idle_since_hint().await?)
            .and_then(at_micros),
    })
}

async fn read_power(manager: &ManagerProxy<'_>) -> Result<PowerPolicy, bus::Error> {
    let (kind, usec) = manager.scheduled_shutdown().await?;
    Ok(PowerPolicy {
        power_key: manager.handle_power_key().await?,
        suspend_key: manager.handle_suspend_key().await?,
        hibernate_key: manager.handle_hibernate_key().await?,
        reboot_key: manager.handle_reboot_key().await?,
        lid_switch: manager.handle_lid_switch().await?,
        chassis: Chassis {
            docked: manager.docked().await?,
            lid_closed: manager.lid_closed().await?,
            on_external_power: manager.on_external_power().await?,
        },
        idle_action: manager.idle_action().await?,
        idle_action_after: Some(manager.idle_action_usec().await?)
            .filter(|usec| *usec > 0 && *usec < u64::MAX)
            .map(Duration::from_micros),
        sleep_order: manager.sleep_operation().await?,
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
