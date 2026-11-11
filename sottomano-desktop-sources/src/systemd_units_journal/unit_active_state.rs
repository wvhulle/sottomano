use std::{pin::pin, sync::Arc};

use futures::{Stream, StreamExt, stream};
use tokio::select;

use super::{
    ManagerProxy, SystemdUnitOnBus, UNIT_INTERFACE, UnitProxy, connect_systemd_bus,
    signals_matching, systemd_signal,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{Availability, connected_until_failure, read_once_then_per_burst},
    systemd_units_journal::SystemdUnitScope,
};

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

pub fn unit_running(unit: SystemdUnitOnBus) -> impl Stream<Item = Availability> + Send {
    unit_activity(unit).map(|activity| match activity {
        Ok(UnitActivity::Running) => Availability::Available,
        Ok(_) => Availability::Unavailable,
        Err(activity_error) => {
            tracing::warn!("systemd unit activity: {activity_error}");
            Availability::Unavailable
        }
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
    let unit_path = manager.load_unit(&unit).await?;
    let unit = UnitProxy::builder(&connection)
        .path(unit_path.clone())?
        .build()
        .await?;
    let changes = signals_matching(
        &connection,
        [systemd_signal()?
            .path(unit_path)?
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .arg(0, UNIT_INTERFACE)?
            .build()],
    )
    .await?
    .map(drop);
    Ok(read_once_then_per_burst(changes, move || {
        let unit = unit.clone();
        async move {
            unit.active_state()
                .await
                .map(|state| UnitActivity::from(state.as_str()))
        }
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitControl {
    pub unit_scope: SystemdUnitScope,
    pub unit: String,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum UnitChangeFailure {
    #[error("the job ended {job_result}")]
    JobEnded { job_result: String },
    #[error("the systemd call failed: {0}")]
    Bus(Arc<zbus::Error>),
}

impl UnitControl {
    pub async fn apply(self, change: UnitChange) -> Result<(), UnitChangeFailure> {
        let job_result = self
            .job_result(change)
            .await
            .map_err(|bus_error| UnitChangeFailure::Bus(Arc::new(bus_error)))?;
        if job_result == JOB_DONE {
            Ok(())
        } else {
            Err(UnitChangeFailure::JobEnded { job_result })
        }
    }

    async fn job_result(&self, change: UnitChange) -> zbus::Result<String> {
        let connection = connect_systemd_bus(self.unit_scope).await?;
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
