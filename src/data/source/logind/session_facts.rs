use std::time::Duration;

use chrono::{DateTime, Utc};
use humantime::format_duration;
use zbus::{blocking, proxy, zvariant::OwnedObjectPath};

use crate::{
    data::{
        empty_text::non_empty,
        source::{
            PolledSource, Source,
            bus::{self, call},
            hostname::{at_micros, entry, hostname_blocking},
            logind::{ManagerProxy, ManagerProxyBlocking, session_id},
        },
    },
    widgets::{
        localized_time::TimePart,
        tooltip_body::{Cell, KeyValue, Section},
    },
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
    #[zbus(property, name = "TTY")]
    fn tty(&self) -> zbus::Result<String>;
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
    fn remote(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn remote_host(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn remote_user(&self) -> zbus::Result<String>;
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
    pub host: String,
    pub seat: Option<String>,
    pub login_manager: String,
    pub desktop: Option<String>,
    pub session_type: String,
}

impl SessionIdentity {
    pub fn section(&self) -> Section {
        Section::dictionary([
            KeyValue::new("User", format!("{} ({})", self.user, self.uid)),
            KeyValue::new("Host", self.host.clone()),
        ])
        .titled("Account")
    }
}

pub fn identity() -> Option<SessionIdentity> {
    let connection = blocking::Connection::system().ok()?;
    let manager = ManagerProxyBlocking::new(&connection).ok()?;
    let path = manager.get_session(&session_id()).ok()?;
    let session = SessionProxyBlocking::builder(&connection)
        .path(path)
        .ok()?
        .build()
        .ok()?;
    Some(SessionIdentity {
        user: session.name().ok().and_then(non_empty)?,
        uid: session.user().ok()?.0,
        host: hostname_blocking()?,
        seat: session.seat().ok().and_then(|(seat, _)| non_empty(seat)),
        login_manager: session.service().ok()?,
        desktop: session.desktop().ok().and_then(non_empty),
        session_type: session.session_type().ok()?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub seat: Option<String>,
    pub virtual_terminal: Option<u32>,
    pub tty: Option<String>,
    pub session_type: String,
    pub class: String,
    pub login_manager: String,
    pub scope: String,
    pub leader: u32,
    pub state: String,
    pub remote_host: Option<String>,
    pub remote_user: Option<String>,
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
    pub inhibitors: u64,
    pub block_inhibited: Option<String>,
    pub delay_inhibited: Option<String>,
    pub sessions: u64,
    pub scheduled_shutdown: Option<ScheduledShutdown>,
    pub preparing_for_shutdown: bool,
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

impl Source for SessionInfo {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "logind session";
}

impl PolledSource for SessionInfo {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let connection = system_bus().await?;
        let manager = call(ManagerProxy::new(&connection).await)?;
        let path = call(manager.get_session(&session_id()).await)?;
        let session_builder = call(SessionProxy::builder(&connection).path(path))?;
        let session = call(session_builder.build().await)?;
        read_session(&session).await
    }
}

impl Source for PowerPolicy {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "logind power policy";
}

impl PolledSource for PowerPolicy {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let connection = system_bus().await?;
        let manager = call(ManagerProxy::new(&connection).await)?;
        read_power(&manager).await
    }
}

async fn system_bus() -> Result<zbus::Connection, bus::Error> {
    zbus::Connection::system()
        .await
        .map_err(|e| bus::Error::Connect(e.to_string()))
}

fn optional_text(reply: zbus::Result<String>) -> Result<Option<String>, bus::Error> {
    call(reply).map(non_empty)
}

async fn read_session(session: &SessionProxy<'_>) -> Result<SessionInfo, bus::Error> {
    let remote = call(session.remote().await)?;
    let idle = call(session.idle_hint().await)?;
    let since = call(session.timestamp().await)?;
    Ok(SessionInfo {
        id: call(session.id().await)?,
        seat: non_empty(call(session.seat().await)?.0),
        virtual_terminal: Some(call(session.vt_nr().await)?).filter(|vt| *vt > 0),
        tty: optional_text(session.tty().await)?,
        session_type: call(session.session_type().await)?,
        class: call(session.class().await)?,
        login_manager: call(session.service().await)?,
        scope: call(session.scope().await)?,
        leader: call(session.leader().await)?,
        state: call(session.state().await)?,
        remote_host: optional_text(session.remote_host().await)?.filter(|_| remote),
        remote_user: optional_text(session.remote_user().await)?.filter(|_| remote),
        since: at_micros(since).ok_or(bus::Error::Absent)?,
        idle_since: idle
            .then_some(call(session.idle_since_hint().await)?)
            .and_then(at_micros),
    })
}

async fn read_power(manager: &ManagerProxy<'_>) -> Result<PowerPolicy, bus::Error> {
    let (kind, usec) = call(manager.scheduled_shutdown().await)?;
    Ok(PowerPolicy {
        power_key: call(manager.handle_power_key().await)?,
        suspend_key: call(manager.handle_suspend_key().await)?,
        hibernate_key: call(manager.handle_hibernate_key().await)?,
        reboot_key: call(manager.handle_reboot_key().await)?,
        lid_switch: call(manager.handle_lid_switch().await)?,
        chassis: Chassis {
            docked: call(manager.docked().await)?,
            lid_closed: call(manager.lid_closed().await)?,
            on_external_power: call(manager.on_external_power().await)?,
        },
        idle_action: call(manager.idle_action().await)?,
        idle_action_after: Some(call(manager.idle_action_usec().await)?)
            .filter(|usec| *usec > 0 && *usec < u64::MAX)
            .map(Duration::from_micros),
        sleep_order: call(manager.sleep_operation().await)?,
        inhibitors: call(manager.n_current_inhibitors().await)?,
        block_inhibited: optional_text(manager.block_inhibited().await)?,
        delay_inhibited: optional_text(manager.delay_inhibited().await)?,
        sessions: call(manager.n_current_sessions().await)?,
        scheduled_shutdown: non_empty(kind).map(|kind| ScheduledShutdown {
            kind,
            at: at_micros(usec),
        }),
        preparing_for_shutdown: call(manager.preparing_for_shutdown().await)?,
    })
}

fn yes_no(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

impl SessionInfo {
    pub fn sections(&self) -> Vec<Section> {
        let seat = self.seat.clone().map(|seat| match self.virtual_terminal {
            Some(vt) => format!("{seat}, VT {vt}"),
            None => seat,
        });
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Login manager", self.login_manager.clone())),
                    Some(KeyValue::new("Session id", self.id.clone())),
                    Some(KeyValue::new(
                        "Kind",
                        format!("{} {}", self.session_type, self.class),
                    )),
                    Some(KeyValue::new("State", self.state.clone())),
                    Some(KeyValue::new(
                        "Logged in",
                        Cell::Time {
                            at: self.since,
                            part: TimePart::DateAndTime,
                        },
                    )),
                    self.idle_since.map(|at| {
                        KeyValue::new(
                            "Idle since",
                            Cell::Time {
                                at,
                                part: TimePart::TimeOfDay,
                            },
                        )
                    }),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Login session"),
            Section::dictionary(
                [
                    entry("Seat", seat),
                    entry("TTY", self.tty.clone()),
                    Some(KeyValue::new("Scope", self.scope.clone())),
                    Some(KeyValue::new("Leader", self.leader.to_string())),
                    entry("Remote host", self.remote_host.clone()),
                    entry("Remote user", self.remote_user.clone()),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Where"),
        ]
    }
}

impl PowerPolicy {
    pub fn sections(&self) -> Vec<Section> {
        let idle_action = match self.idle_action_after {
            Some(after) if self.idle_action != "ignore" => {
                format!("{} after {}", self.idle_action, format_duration(after))
            }
            _ => self.idle_action.clone(),
        };
        let inhibitor_classes = [
            self.block_inhibited
                .as_deref()
                .map(|blocked| format!("block {blocked}")),
            self.delay_inhibited
                .as_deref()
                .map(|delayed| format!("delay {delayed}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("; ");
        let inhibitors = if inhibitor_classes.is_empty() {
            self.inhibitors.to_string()
        } else {
            format!("{} ({inhibitor_classes})", self.inhibitors)
        };
        let scheduled = self.scheduled_shutdown.as_ref().map(|shutdown| {
            let key = match shutdown.kind.as_str() {
                "poweroff" => "Power off scheduled",
                "reboot" => "Reboot scheduled",
                "halt" => "Halt scheduled",
                _ => "Scheduled shutdown",
            };
            match shutdown.at {
                Some(at) => KeyValue::new(
                    key,
                    Cell::Time {
                        at,
                        part: TimePart::DateAndTime,
                    },
                ),
                None => KeyValue::new(key, shutdown.kind.clone()),
            }
        });
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Power key", self.power_key.clone())),
                    Some(KeyValue::new("Suspend key", self.suspend_key.clone())),
                    Some(KeyValue::new("Hibernate key", self.hibernate_key.clone())),
                    Some(KeyValue::new("Reboot key", self.reboot_key.clone())),
                    Some(KeyValue::new("Lid switch", self.lid_switch.clone())),
                    Some(KeyValue::new("Idle action", idle_action)),
                    entry("Sleep order", non_empty(self.sleep_order.join(", "))),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Keys and lid"),
            Section::dictionary(
                [
                    Some(KeyValue::new("Docked", yes_no(self.chassis.docked))),
                    Some(KeyValue::new("Lid closed", yes_no(self.chassis.lid_closed))),
                    Some(KeyValue::new(
                        "External power",
                        yes_no(self.chassis.on_external_power),
                    )),
                    Some(KeyValue::new("Inhibitors", inhibitors)),
                    Some(KeyValue::new("Sessions", self.sessions.to_string())),
                    scheduled,
                    self.preparing_for_shutdown
                        .then(|| KeyValue::new("Preparing for shutdown", "yes")),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Right now"),
        ]
    }
}
