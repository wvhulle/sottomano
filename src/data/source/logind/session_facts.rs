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
    pub host: String,
}

impl SessionIdentity {
    pub fn section(&self) -> Section {
        Section::dictionary([
            KeyValue::new("User", self.user.clone()),
            KeyValue::new("Host", self.host.clone()),
        ])
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
        host: hostname_blocking()?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: Option<String>,
    pub user: Option<String>,
    pub uid: Option<u32>,
    pub seat: Option<String>,
    pub virtual_terminal: Option<u32>,
    pub tty: Option<String>,
    pub session_type: Option<String>,
    pub class: Option<String>,
    pub desktop: Option<String>,
    pub service: Option<String>,
    pub scope: Option<String>,
    pub leader: Option<u32>,
    pub state: Option<String>,
    pub remote_host: Option<String>,
    pub remote_user: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub idle_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerPolicy {
    pub power_key: Option<String>,
    pub suspend_key: Option<String>,
    pub hibernate_key: Option<String>,
    pub reboot_key: Option<String>,
    pub lid_switch: Option<String>,
    pub docked: Option<bool>,
    pub lid_closed: Option<bool>,
    pub on_external_power: Option<bool>,
    pub idle_action: Option<String>,
    pub idle_action_after: Option<Duration>,
    pub sleep_order: Vec<String>,
    pub inhibitors: Option<u64>,
    pub block_inhibited: Option<String>,
    pub delay_inhibited: Option<String>,
    pub sessions: Option<u64>,
    pub scheduled_shutdown: Option<ScheduledShutdown>,
    pub preparing_for_shutdown: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledShutdown {
    pub kind: String,
    pub at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFacts {
    pub session: SessionInfo,
    pub power: PowerPolicy,
}

impl Source for SessionFacts {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "logind session";
}

impl PolledSource for SessionFacts {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let connection = zbus::Connection::system()
            .await
            .map_err(|e| bus::Error::Connect(e.to_string()))?;
        let manager = call(ManagerProxy::new(&connection).await)?;
        let path = call(manager.get_session(&session_id()).await)?;
        let session_builder = call(SessionProxy::builder(&connection).path(path))?;
        let session = call(session_builder.build().await)?;
        Ok(Self {
            session: read_session(&session).await,
            power: read_power(&manager).await,
        })
    }
}

fn text(reply: zbus::Result<String>) -> Option<String> {
    reply.ok().and_then(non_empty)
}

async fn read_session(session: &SessionProxy<'_>) -> SessionInfo {
    let remote = session.remote().await.unwrap_or(false);
    let idle = session.idle_hint().await.unwrap_or(false);
    SessionInfo {
        id: text(session.id().await),
        user: text(session.name().await),
        uid: session.user().await.ok().map(|(uid, _)| uid),
        seat: session
            .seat()
            .await
            .ok()
            .and_then(|(seat, _)| non_empty(seat)),
        virtual_terminal: session.vt_nr().await.ok().filter(|vt| *vt > 0),
        tty: text(session.tty().await),
        session_type: text(session.session_type().await),
        class: text(session.class().await),
        desktop: text(session.desktop().await),
        service: text(session.service().await),
        scope: text(session.scope().await),
        leader: session.leader().await.ok().filter(|pid| *pid > 0),
        state: text(session.state().await),
        remote_host: text(session.remote_host().await).filter(|_| remote),
        remote_user: text(session.remote_user().await).filter(|_| remote),
        since: session.timestamp().await.ok().and_then(at_micros),
        idle_since: session
            .idle_since_hint()
            .await
            .ok()
            .filter(|_| idle)
            .and_then(at_micros),
    }
}

async fn read_power(manager: &ManagerProxy<'_>) -> PowerPolicy {
    PowerPolicy {
        power_key: text(manager.handle_power_key().await),
        suspend_key: text(manager.handle_suspend_key().await),
        hibernate_key: text(manager.handle_hibernate_key().await),
        reboot_key: text(manager.handle_reboot_key().await),
        lid_switch: text(manager.handle_lid_switch().await),
        docked: manager.docked().await.ok(),
        lid_closed: manager.lid_closed().await.ok(),
        on_external_power: manager.on_external_power().await.ok(),
        idle_action: text(manager.idle_action().await),
        idle_action_after: manager
            .idle_action_usec()
            .await
            .ok()
            .filter(|usec| *usec > 0 && *usec < u64::MAX)
            .map(Duration::from_micros),
        sleep_order: manager.sleep_operation().await.unwrap_or_default(),
        inhibitors: manager.n_current_inhibitors().await.ok(),
        block_inhibited: text(manager.block_inhibited().await),
        delay_inhibited: text(manager.delay_inhibited().await),
        sessions: manager.n_current_sessions().await.ok(),
        scheduled_shutdown: manager
            .scheduled_shutdown()
            .await
            .ok()
            .and_then(|(kind, usec)| {
                non_empty(kind).map(|kind| ScheduledShutdown {
                    kind,
                    at: at_micros(usec),
                })
            }),
        preparing_for_shutdown: manager.preparing_for_shutdown().await.unwrap_or(false),
    }
}

fn time(at: Option<DateTime<Utc>>, part: TimePart) -> Option<Cell> {
    at.map(|at| Cell::Time { at, part })
}

fn yes_no(flag: Option<bool>) -> Option<&'static str> {
    flag.map(|flag| if flag { "yes" } else { "no" })
}

impl SessionInfo {
    fn section(&self) -> Section {
        let user = self.user.clone().map(|name| match self.uid {
            Some(uid) => format!("{name} ({uid})"),
            None => name,
        });
        let seat = self.seat.clone().map(|seat| match self.virtual_terminal {
            Some(vt) => format!("{seat}, VT {vt}"),
            None => seat,
        });
        Section::dictionary(
            [
                entry("Id", self.id.clone()),
                entry("User", user),
                entry("Seat", seat),
                entry("TTY", self.tty.clone()),
                entry("Type", self.session_type.clone()),
                entry("Class", self.class.clone()),
                entry("Desktop", self.desktop.clone()),
                entry("Started by", self.service.clone()),
                entry("Scope", self.scope.clone()),
                entry("Leader", self.leader.map(|pid| pid.to_string())),
                entry("State", self.state.clone()),
                entry("Since", time(self.since, TimePart::DateAndTime)),
                entry("Idle since", time(self.idle_since, TimePart::TimeOfDay)),
                entry("Remote host", self.remote_host.clone()),
                entry("Remote user", self.remote_user.clone()),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Session")
    }
}

impl PowerPolicy {
    fn section(&self) -> Section {
        let idle_action = self
            .idle_action
            .clone()
            .map(|action| match self.idle_action_after {
                Some(after) if action != "ignore" => {
                    format!("{action} after {}", format_duration(after))
                }
                _ => action,
            });
        let inhibitors = self.inhibitors.map(|count| {
            let classes = [
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
            if classes.is_empty() {
                count.to_string()
            } else {
                format!("{count} ({classes})")
            }
        });
        let scheduled = self
            .scheduled_shutdown
            .as_ref()
            .map(|shutdown| match shutdown.at {
                Some(at) => Cell::Time {
                    at,
                    part: TimePart::DateAndTime,
                },
                None => Cell::from(shutdown.kind.clone()),
            });
        let scheduled_key = self.scheduled_shutdown.as_ref().map_or(
            "Scheduled shutdown",
            |shutdown| match shutdown.kind.as_str() {
                "poweroff" => "Power off scheduled",
                "reboot" => "Reboot scheduled",
                "halt" => "Halt scheduled",
                _ => "Scheduled shutdown",
            },
        );
        Section::dictionary(
            [
                entry("Power key", self.power_key.clone()),
                entry("Suspend key", self.suspend_key.clone()),
                entry("Hibernate key", self.hibernate_key.clone()),
                entry("Reboot key", self.reboot_key.clone()),
                entry("Lid switch", self.lid_switch.clone()),
                entry("Docked", yes_no(self.docked)),
                entry("Lid closed", yes_no(self.lid_closed)),
                entry("External power", yes_no(self.on_external_power)),
                entry("Idle action", idle_action),
                entry("Sleep order", non_empty(self.sleep_order.join(", "))),
                entry("Inhibitors", inhibitors),
                entry("Sessions", self.sessions.map(|count| count.to_string())),
                entry(scheduled_key, scheduled),
                entry(
                    "Preparing for shutdown",
                    self.preparing_for_shutdown.then_some("yes"),
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Power keys and lid")
    }
}

impl SessionFacts {
    pub fn sections(&self) -> Vec<Section> {
        vec![self.session.section(), self.power.section()]
    }
}
