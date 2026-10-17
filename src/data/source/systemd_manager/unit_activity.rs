use std::{convert::Infallible, future::ready, marker::PhantomData};

use iced::futures::{Stream, StreamExt, stream};

use super::{ManagerProxy, UnitProxy, UnitScope, connect};
use crate::{
    data::source::{Delivery, LiveSource, Source},
    message_subscriptions::restarting,
};

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
    pub fn running(self) -> bool {
        self == Self::Running
    }

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
    pub fn pending_activity(self) -> UnitActivity {
        match self {
            Self::Start => UnitActivity::Starting,
            Self::Stop => UnitActivity::Stopping,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitActiveState<S: UnitScope> {
    pub activity: UnitActivity,
    scope: PhantomData<S>,
}

impl<S: UnitScope> From<UnitActivity> for UnitActiveState<S> {
    fn from(activity: UnitActivity) -> Self {
        Self {
            activity,
            scope: PhantomData,
        }
    }
}

impl<S: UnitScope> Source for UnitActiveState<S> {
    type Error = Infallible;
    type Key = String;

    const NAME: &'static str = "systemd unit activity";
}

impl<S: UnitScope> LiveSource for UnitActiveState<S> {
    fn events(unit: String) -> impl Stream<Item = Delivery<Self>> + Send {
        unit_activity::<S>(unit).map(|activity| Delivery::Reading(Ok(activity.into())))
    }
}

pub fn unit_running<S: UnitScope>(unit: String) -> impl Stream<Item = bool> + Send {
    unit_activity::<S>(unit).map(UnitActivity::running)
}

fn unit_activity<S: UnitScope>(unit: String) -> impl Stream<Item = UnitActivity> + Send {
    restarting("systemd unit activity", move || {
        activity_changes(S::PER_USER, unit.clone())
    })
}

async fn activity_changes(
    per_user: bool,
    unit: String,
) -> zbus::Result<impl Stream<Item = UnitActivity> + Send> {
    let connection = connect(per_user).await?;
    let manager = ManagerProxy::new(&connection).await?;
    manager.subscribe().await?;
    let unit = UnitProxy::builder(&connection)
        .path(manager.load_unit(&unit).await?)?
        .build()
        .await?;
    let current = unit.active_state().await?;
    let changes = unit
        .receive_active_state_changed()
        .await
        .then(|change| async move { change.get().await.ok() })
        .filter_map(ready);
    Ok(stream::iter([current])
        .chain(changes)
        .map(|state| UnitActivity::from(state.as_str())))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitControl {
    pub per_user: bool,
    pub unit: String,
}

impl UnitControl {
    pub async fn accepts(self, change: UnitChange) -> bool {
        self.request(change)
            .await
            .inspect_err(|e| tracing::warn!("systemd: {change:?} {} failed: {e}", self.unit))
            .is_ok()
    }

    async fn request(&self, change: UnitChange) -> zbus::Result<()> {
        let connection = connect(self.per_user).await?;
        let manager = ManagerProxy::new(&connection).await?;
        match change {
            UnitChange::Start => manager.start_unit(&self.unit, "replace").await?,
            UnitChange::Stop => manager.stop_unit(&self.unit, "replace").await?,
        };
        Ok(())
    }
}
