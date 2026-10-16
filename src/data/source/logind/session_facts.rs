use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use humantime::format_duration;
use iced::alignment::Horizontal;
use procfs::process::Process as Proc;
use zbus::{proxy, zvariant::OwnedObjectPath};

use crate::{
    data::{
        empty_text::non_empty,
        source::{
            PolledSource, Source,
            bus,
            hostname::{at_micros, entry},
            logind::{
                InhibitorRow, ManagerProxy, greetd_greeter::greeter_program,
                session_id,
            },
        },
    },
    widgets::{
        localized_time::TimePart,
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord},
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
    pub seat: Option<String>,
    pub console: Option<String>,
    pub virtual_terminal: Option<u32>,
    pub remote_user: Option<String>,
    pub remote_host: Option<String>,
    pub login_manager: String,
    pub desktop: Option<String>,
    pub session_type: String,
}

impl Source for SessionIdentity {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "logind session identity";
}

impl PolledSource for SessionIdentity {
    const INTERVAL: Duration = Duration::from_hours(1);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let session = own_session().await?;
        let remote = session.remote().await?;
        Ok(SessionIdentity {
            user: non_empty(session.name().await?).ok_or(bus::Error::Absent)?,
            uid: session.user().await?.0,
            seat: session.seat().await.ok().and_then(|(seat, _)| non_empty(seat)),
            console: session.tty().await.ok().and_then(non_empty),
            virtual_terminal: session.vt_nr().await.ok().filter(|vt| *vt > 0),
            remote_user: session
                .remote_user()
                .await
                .ok()
                .and_then(non_empty)
                .filter(|_| remote),
            remote_host: session
                .remote_host()
                .await
                .ok()
                .and_then(non_empty)
                .filter(|_| remote),
            login_manager: session.service().await?,
            desktop: session.desktop().await.ok().and_then(non_empty),
            session_type: session.session_type().await?,
        })
    }
}

async fn own_session() -> Result<SessionProxy<'static>, bus::Error> {
    let connection = system_bus().await?;
    let manager = ManagerProxy::new(&connection).await?;
    let path = manager.get_session(&session_id()).await?;
    let session_builder = SessionProxy::builder(&connection).path(path)?;
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

    fn table_record(&self) -> TableRecord<2> {
        TableRecord::new(
            self.who.clone(),
            [
                Some(self.operation.clone().into()),
                self.why.clone().map(Cell::from),
            ],
        )
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

impl Source for SessionInfo {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "logind session";
}

impl PolledSource for SessionInfo {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        read_session(&own_session().await?).await
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
        let manager = ManagerProxy::new(&connection).await?;
        read_power(&manager).await
    }
}

async fn system_bus() -> Result<zbus::Connection, bus::Error> {
    zbus::Connection::system()
        .await
        .map_err(|e| bus::Error::Connect(e.to_string()))
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
        inhibitor_locks: manager.list_inhibitors().await?
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

fn yes_no(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

impl SessionInfo {
    pub fn sections(&self) -> Vec<Section> {
        vec![
            Section::dictionary(entry("Greeter", self.greeter.clone())).titled((
                "Login screen shown before this session",
                "Drew the password prompt in its own short session, closed once you logged in.",
            )),
            Section::dictionary(
                [
                    Some(KeyValue::new("Session number", self.id.clone())),
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
            .titled((
                "Login session tracked by logind",
                "logind tracks this login and every process started from it.",
            )),
            Section::dictionary(
                [
                    entry("Started by", self.leader_name.clone()),
                    Some(KeyValue::new("Starter process id", self.leader.to_string())),
                    Some(KeyValue::new("Processes kept in", self.scope.clone())),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Processes belonging to this login",
                "Everything launched in this login lives in one systemd scope.",
            )),
        ]
        .into_iter()
        .filter(|section| !section.body().is_empty())
        .collect()
    }
}

impl PowerPolicy {
    pub fn sections(&self) -> Vec<Section> {
        let idle_after = self
            .idle_action_after
            .filter(|_| self.idle_action != "ignore")
            .map(|after| KeyValue::new("Idle after", format_duration(after).to_string()));
        let sleep_tries = self
            .sleep_order
            .iter()
            .enumerate()
            .map(|(position, operation)| {
                KeyValue::new(format!("Sleep try {}", position + 1), operation.clone())
            });
        let locks_in_mode = |mode: InhibitMode| {
            self.inhibitor_locks
                .iter()
                .filter(move |lock| lock.mode == mode)
                .map(InhibitorLock::table_record)
        };
        let lock_headings = [
            ColumnHeading::new("Holds").align_x(Horizontal::Left),
            ColumnHeading::new("Why").align_x(Horizontal::Left),
        ];
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
                    Some(KeyValue::new("Idle action", self.idle_action.clone())),
                    idle_after,
                ]
                .into_iter()
                .flatten()
                .chain(sleep_tries),
            )
            .titled((
                "What the keys and lid trigger",
                "What logind does when these are pressed or closed.",
            )),
            Section::dictionary(
                [
                    Some(KeyValue::new("Docked", yes_no(self.chassis.docked))),
                    Some(KeyValue::new("Lid closed", yes_no(self.chassis.lid_closed))),
                    Some(KeyValue::new(
                        "External power",
                        yes_no(self.chassis.on_external_power),
                    )),
                    Some(KeyValue::new("Sessions", self.sessions.to_string())),
                    scheduled,
                    self.preparing_for_shutdown
                        .then(|| KeyValue::new("Preparing for shutdown", "yes")),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Current state of the machine",
                "Conditions that change which of these actions applies.",
            )),
            Section::column_table(lock_headings, locks_in_mode(InhibitMode::Delay)).titled((
                "Apps briefly holding off sleep or shutdown",
                "Each gets a few seconds to clean up first; the action still happens.",
            )),
            Section::column_table(lock_headings, locks_in_mode(InhibitMode::Block)).titled((
                "Apps that took over from logind",
                "logind skips its own action; the app handles or prevents it.",
            )),
        ]
        .into_iter()
        .filter(|section| !section.body().is_empty())
        .collect()
    }
}
