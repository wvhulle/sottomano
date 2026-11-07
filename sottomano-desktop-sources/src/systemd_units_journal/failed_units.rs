use std::time::SystemTime;

use futures::{Stream, StreamExt, future::join_all};
use tokio::task::spawn_blocking;
use zbus::{proxy::CacheProperties, zvariant::OwnedObjectPath};

use super::{
    ManagerProxy, ServiceProxy, SystemdOnBus, SystemdUnitScope, UNIT_INTERFACE, UnitRow,
    manager_signal, signals_matching, state_changed_at, systemd_signal,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{connected_until_failure, read_once_then_per_burst},
    systemd_units_journal::{
        journal_coredumps::CoredumpEntry, journal_record_fields::JournalPriority,
    },
};

pub const UNIT_FAILURE_PRIORITY: JournalPriority = JournalPriority::Warning;

const FAILED_STATE: &str = "failed";
const CORE_DUMP_RESULT: &str = "core-dump";

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
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    #[must_use]
    pub fn follow_log_args(&self) -> Vec<String> {
        self.scope
            .follow_units_log_args(self.units.iter().map(|unit| unit.name.as_str()))
    }
}

#[derive(Debug, Clone)]
pub struct FailedUnitRestart {
    pub unit_scope: SystemdUnitScope,
    pub unit: String,
}

impl SnapshotSource for FailedUnits {
    type Error = zbus::Error;
    type StreamInputs = SystemdOnBus;

    const NAME: &'static str = "Failed systemd units";

    fn delivery_stream(systemd: SystemdOnBus) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let scope = systemd.scope;
        connected_until_failure(watch_failed(systemd.connection(), scope))
            .map(move |listed| listed.map(|units| FailedUnits { units, scope }))
            .map(SnapshotDelivery::live_push)
    }
}

async fn watch_failed(
    connection: zbus::Connection,
    unit_scope: SystemdUnitScope,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<FailedUnitInfo>>>> {
    let manager = ManagerProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    manager.ensure_subscribed().await?;
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
    .map(drop);
    Ok(read_once_then_per_burst(unit_signals, move || {
        let manager = manager.clone();
        async move { failed_units(&manager, unit_scope).await }
    }))
}

async fn failed_units(
    manager: &ManagerProxy<'static>,
    unit_scope: SystemdUnitScope,
) -> zbus::Result<Vec<FailedUnitInfo>> {
    let rows = manager.list_units_filtered(&[FAILED_STATE]).await?;
    let connection = manager.inner().connection();
    Ok(join_all(
        rows.into_iter()
            .map(|row| failed_unit(connection, unit_scope, row)),
    )
    .await)
}

async fn failed_unit(
    connection: &zbus::Connection,
    unit_scope: SystemdUnitScope,
    row: UnitRow,
) -> FailedUnitInfo {
    let service_outcome = service_outcome(connection, row.unit_path.clone()).await;
    let crashed = service_outcome
        .as_ref()
        .is_some_and(|outcome| outcome.result == CORE_DUMP_RESULT);
    let crash = if crashed {
        let name = row.name.clone();
        spawn_blocking(move || CoredumpEntry::latest_of_unit(unit_scope, &name))
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
