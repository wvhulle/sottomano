use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use futures::Stream;
use procfs::process::Process as Proc;
use zbus::{proxy, zvariant::OwnedObjectPath};

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
        dbus_desktop_buses::{self, DesktopBuses},
    },
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
    pub virtual_terminal: Option<NonZeroU32>,
    pub login_manager: String,
    pub desktop: Option<String>,
    pub session_type: String,
}

impl SnapshotSource for SessionIdentity {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Session identity";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, identity_changes)
    }
}

async fn identity_changes(
    buses: DesktopBuses,
) -> Result<
    impl Stream<Item = Result<SessionIdentity, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let session = own_session(&buses.system).await?;
    let identity = SessionIdentity {
        user: non_empty(session.name().await?).ok_or(dbus_desktop_buses::Error::Absent)?,
        uid: session.user().await?.0,
        seat: non_empty(session.seat().await?.0).ok_or(dbus_desktop_buses::Error::Absent)?,
        virtual_terminal: NonZeroU32::new(session.vt_nr().await?),
        login_manager: session.service().await?,
        desktop: session.desktop().await.ok().and_then(non_empty),
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

async fn own_session(
    system: &zbus::Connection,
) -> Result<SessionProxy<'static>, dbus_desktop_buses::Error> {
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

impl SnapshotSource for SessionInfo {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Login session";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, session_changes)
    }
}

async fn session_changes(
    buses: DesktopBuses,
) -> Result<
    impl Stream<Item = Result<SessionInfo, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let session = own_session(&buses.system).await?;
    let leader = session.leader().await?;
    let info = SessionInfo {
        id: session.id().await?,
        scope: session.scope().await?,
        leader,
        leader_name: process_name(leader),
        greeter: greeter_program(leader),
        state: session.state().await?,
        since: at_micros(session.timestamp().await?).ok_or(dbus_desktop_buses::Error::Absent)?,
        idle_since: None,
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

fn process_name(pid: u32) -> Option<String> {
    let process = Proc::new(i32::try_from(pid).ok()?).ok()?;
    non_empty(process.stat().ok()?.comm)
}
