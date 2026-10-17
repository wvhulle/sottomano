use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    time::SystemTime,
};

use iced::futures::{Stream, StreamExt, future::join_all, stream};
use tokio::task::spawn_blocking;
use zbus::{
    names::MemberName,
    proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use super::{
    ManagerProxy, SystemdOnBus, SystemdUnitScope, UnitRow, connect, manager_signal,
    signals_matching, state_changed_at, systemd_signal,
};
use crate::{
    data::source::{
        DeliveryCadence, ReadingSource, SourceDelivery, coredumps::CoredumpEntry,
        journal::JournalPriority,
    },
    message_subscriptions::connected_until_failure,
};

pub const UNIT_FAILURE_PRIORITY: JournalPriority = JournalPriority::Warning;

const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";
const ACTIVE_STATE_PROPERTY: &str = "ActiveState";
const FAILED_STATE: &str = "failed";
const CORE_DUMP_RESULT: &str = "core-dump";

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
    pub crash: Option<CoredumpEntry>,
}

#[derive(Debug, Clone)]
pub struct FailedUnits {
    pub units: Vec<FailedUnitInfo>,
    pub scope: SystemdUnitScope,
}

impl FailedUnits {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn follow_log_args(&self) -> Vec<String> {
        self.scope
            .follow_units_log_args(self.units.iter().map(|unit| unit.name.as_str()))
    }
}

#[derive(Debug, Clone)]
pub struct FailedUnitRestart {
    pub per_user: bool,
    pub unit: String,
}

impl FailedUnitRestart {
    pub async fn apply(self) {
        if let Err(e) = self.restart().await {
            tracing::warn!("systemd: restarting {} failed: {e}", self.unit);
        }
    }

    async fn restart(&self) -> zbus::Result<()> {
        let connection = connect(self.per_user).await?;
        ManagerProxy::new(&connection)
            .await?
            .restart_unit(&self.unit, "replace")
            .await?;
        Ok(())
    }
}

impl ReadingSource for FailedUnits {
    type Error = zbus::Error;
    type StreamInputs = SystemdOnBus;

    const NAME: &'static str = "failed systemd units";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(systemd: SystemdOnBus) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let scope = systemd.scope;
        connected_until_failure(watch_failed(systemd.connection(), scope.per_user()))
            .map(move |listed| listed.map(|units| FailedUnits { units, scope }))
            .map(SourceDelivery::Reading)
    }
}

async fn watch_failed(
    connection: zbus::Connection,
    user: bool,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<FailedUnitInfo>>>> {
    let manager = ManagerProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    manager.subscribe().await?;
    let unit_signals = signals_matching(
        &connection,
        [
            systemd_signal()?
                .interface("org.freedesktop.DBus.Properties")?
                .member("PropertiesChanged")?
                .arg(0, UNIT_INTERFACE)?
                .build(),
            manager_signal("UnitRemoved")?,
            manager_signal("Reloading")?,
        ],
    )
    .await?
    .ready_chunks(64)
    .map(|messages| messages.into_iter().filter_map(Result::ok).collect());
    Ok(stream::unfold(
        FailedUnitWatch {
            manager,
            per_user: user,
            unit_signals: Box::pin(unit_signals),
            last_listing: None,
        },
        FailedUnitWatch::next_report,
    ))
}

struct FailedUnitWatch {
    manager: ManagerProxy<'static>,
    per_user: bool,
    unit_signals: Pin<Box<dyn Stream<Item = Vec<zbus::Message>> + Send>>,
    last_listing: Option<FailedListing>,
}

impl FailedUnitWatch {
    async fn next_report(mut self) -> Option<(zbus::Result<Vec<FailedUnitInfo>>, Self)> {
        let listed = match self.last_listing.take() {
            None => failed_units(&self.manager, self.per_user).await,
            Some(last_listing) => self.next_listing(last_listing).await?,
        };
        let report = listed.map(|listing| {
            let units = listing.units.clone();
            self.last_listing = Some(listing);
            units
        });
        Some((report, self))
    }

    async fn next_listing(
        &mut self,
        last_listing: FailedListing,
    ) -> Option<zbus::Result<FailedListing>> {
        loop {
            let signals = self.unit_signals.next().await?;
            if signals.iter().any(|signal| last_listing.touched_by(signal)) {
                return Some(failed_units(&self.manager, self.per_user).await);
            }
        }
    }
}

struct FailedListing {
    units: Vec<FailedUnitInfo>,
    paths: HashSet<OwnedObjectPath>,
}

impl FailedListing {
    fn touched_by(&self, signal: &zbus::Message) -> bool {
        let header = signal.header();
        let concerns_listed_unit = || {
            header
                .path()
                .is_some_and(|path| self.paths.iter().any(|listed| listed.as_ref() == *path))
        };
        match header.member().map(MemberName::as_str) {
            Some("PropertiesChanged") => match changed_active_state(signal) {
                ActiveStateChange::Unchanged => false,
                ActiveStateChange::To(state) => state == FAILED_STATE || concerns_listed_unit(),
                ActiveStateChange::Unreadable => true,
            },
            Some("UnitRemoved") => signal
                .body()
                .deserialize::<(String, OwnedObjectPath)>()
                .map_or(true, |(_, path)| self.paths.contains(&path)),
            _ => true,
        }
    }
}

enum ActiveStateChange {
    Unchanged,
    To(String),
    Unreadable,
}

fn changed_active_state(signal: &zbus::Message) -> ActiveStateChange {
    let Ok((_, changed, invalidated)) =
        signal
            .body()
            .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
    else {
        return ActiveStateChange::Unreadable;
    };
    match changed.get(ACTIVE_STATE_PROPERTY) {
        Some(value) => String::try_from(value.clone())
            .map_or(ActiveStateChange::Unreadable, ActiveStateChange::To),
        None if invalidated.iter().any(|name| name == ACTIVE_STATE_PROPERTY) => {
            ActiveStateChange::Unreadable
        }
        None => ActiveStateChange::Unchanged,
    }
}

async fn failed_units(
    manager: &ManagerProxy<'static>,
    per_user: bool,
) -> zbus::Result<FailedListing> {
    let rows = manager.list_units_filtered(&[FAILED_STATE]).await?;
    let connection = manager.inner().connection();
    let paths = rows.iter().map(|row| row.unit_path.clone()).collect();
    Ok(FailedListing {
        units: join_all(
            rows.into_iter()
                .map(|row| failed_unit(connection, per_user, row)),
        )
        .await,
        paths,
    })
}

async fn failed_unit(
    connection: &zbus::Connection,
    per_user: bool,
    row: UnitRow,
) -> FailedUnitInfo {
    let service_outcome = service_outcome(connection, row.unit_path.clone()).await;
    let crashed = service_outcome
        .as_ref()
        .is_some_and(|outcome| outcome.result == CORE_DUMP_RESULT);
    let crash = if crashed {
        let name = row.name.clone();
        spawn_blocking(move || CoredumpEntry::latest_of_unit(per_user, &name))
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    FailedUnitInfo {
        at: state_changed_at(connection, row.unit_path).await,
        service_outcome,
        name: row.name,
        crash,
    }
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
