use std::pin::pin;

use futures::{Stream, StreamExt, stream};
use tokio::select;

use super::{ManagerProxy, SystemdUnitOnBus, UnitProxy, connect_systemd_bus};
use crate::{SnapshotDelivery, SnapshotSource, stream_operators::connected_until_failure};

const JOB_DONE: &str = "done";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitActivity {
    Running,
    Starting,
    Stopping,
    Stopped,
}

impl From<&str> for UnitActivity {
    fn from(active_state: &str) -> Self {
        match active_state {
            "active" | "reloading" | "refreshing" => Self::Running,
            "activating" => Self::Starting,
            "deactivating" => Self::Stopping,
            _ => Self::Stopped,
        }
    }
}

impl UnitActivity {
    #[must_use]
    pub fn running(self) -> bool {
        self == Self::Running
    }

    #[must_use]
    pub fn press_change(self) -> UnitChange {
        match self {
            Self::Running | Self::Starting => UnitChange::Stop,
            Self::Stopping | Self::Stopped => UnitChange::Start,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitChange {
    Start,
    Stop,
}

impl UnitChange {
    #[must_use]
    pub fn pending_activity(self) -> UnitActivity {
        match self {
            Self::Start => UnitActivity::Starting,
            Self::Stop => UnitActivity::Stopping,
        }
    }

    #[must_use]
    pub fn settled_activity(self) -> UnitActivity {
        match self {
            Self::Start => UnitActivity::Running,
            Self::Stop => UnitActivity::Stopped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitActiveState {
    pub activity: UnitActivity,
}

impl SnapshotSource for UnitActiveState {
    type Error = zbus::Error;
    type StreamInputs = SystemdUnitOnBus;

    const NAME: &'static str = "Systemd unit activity";

    fn delivery_stream(
        unit: SystemdUnitOnBus,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        unit_activity(unit).map(|activity| {
            SnapshotDelivery::live_push(activity.map(|activity| UnitActiveState { activity }))
        })
    }
}

pub fn unit_running(unit: SystemdUnitOnBus) -> impl Stream<Item = bool> + Send {
    unit_activity(unit).map(|activity| {
        activity
            .inspect_err(|e| tracing::warn!("systemd unit activity: {e}"))
            .is_ok_and(UnitActivity::running)
    })
}

fn unit_activity(unit: SystemdUnitOnBus) -> impl Stream<Item = zbus::Result<UnitActivity>> + Send {
    connected_until_failure(activity_changes(unit.connection(), unit.unit.unit_name))
}

async fn activity_changes(
    connection: zbus::Connection,
    unit: String,
) -> zbus::Result<impl Stream<Item = zbus::Result<UnitActivity>> + Send> {
    let manager = ManagerProxy::new(&connection).await?;
    manager.ensure_subscribed().await?;
    let unit = UnitProxy::builder(&connection)
        .path(manager.load_unit(&unit).await?)?
        .build()
        .await?;
    let current = unit.active_state().await?;
    let changes = unit
        .receive_active_state_changed()
        .await
        .then(|change| async move { change.get().await });
    Ok(stream::iter([Ok(current)])
        .chain(changes)
        .map(|state| state.map(|state| UnitActivity::from(state.as_str()))))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitControl {
    pub per_user: bool,
    pub unit: String,
}

impl UnitControl {
    pub async fn accepts(self, change: UnitChange) -> bool {
        match self.job_result(change).await {
            Ok(result) if result == JOB_DONE => true,
            Ok(result) => {
                tracing::warn!("systemd: {change:?} {} ended {result}", self.unit);
                false
            }
            Err(e) => {
                tracing::warn!("systemd: {change:?} {} failed: {e}", self.unit);
                false
            }
        }
    }

    async fn job_result(&self, change: UnitChange) -> zbus::Result<String> {
        let connection = connect_systemd_bus(self.per_user).await?;
        let manager = ManagerProxy::new(&connection).await?;
        manager.ensure_subscribed().await?;
        let mut removed_jobs = manager.receive_job_removed().await?;
        let mut removed_meanwhile = Vec::new();
        let mut requested = pin!(async {
            match change {
                UnitChange::Start => manager.start_unit(&self.unit, "replace").await,
                UnitChange::Stop => manager.stop_unit(&self.unit, "replace").await,
            }
        });
        let job = loop {
            select! {
                job = &mut requested => break job?,
                Some(removed) = removed_jobs.next() => removed_meanwhile.push(removed),
            }
        };
        let mut removed_jobs = stream::iter(removed_meanwhile).chain(removed_jobs);
        while let Some(removed) = removed_jobs.next().await {
            let removed = removed.args()?;
            if *removed.job() == job {
                return Ok(removed.result().clone());
            }
        }
        Err(zbus::Error::Failure(format!(
            "{} job watch ended",
            self.unit
        )))
    }
}
