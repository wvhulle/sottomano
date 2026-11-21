use futures::{Stream, StreamExt};
use zbus::proxy::CacheProperties;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    system_protocol_clients::{
        dbus_change_signals::change_triggers,
        dbus_desktop_buses::{BusReadError, DesktopBuses},
    },
    systemd_units_journal::{
        ManagerProxy, SystemdTimestamp, UNIT_INTERFACE, manager_signal, realtime_usec_timestamp,
        systemd_signal,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserServiceManager {
    pub version: String,
    pub state: String,
    pub started_at: SystemdTimestamp,
}

impl SnapshotSource for UserServiceManager {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Your systemd instance right now";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, manager_state_changes)
            .map(SnapshotDelivery::unavailable_when_absent)
    }
}

async fn manager_state_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<UserServiceManager, BusReadError>> + Send, BusReadError> {
    let manager = ManagerProxy::builder(&buses.session)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    manager.ensure_subscribed().await?;
    let launched = UserServiceManager {
        version: manager.version().await?,
        state: manager.system_state().await?,
        started_at: realtime_usec_timestamp(manager.userspace_timestamp().await?),
    };
    let state_triggers = change_triggers(
        &buses.session,
        [
            manager_signal("StartupFinished")?,
            manager_signal("JobRemoved")?,
            systemd_signal()?
                .interface("org.freedesktop.DBus.Properties")?
                .member("PropertiesChanged")?
                .arg(0, UNIT_INTERFACE)?
                .build(),
        ],
    )
    .await?;
    Ok(state_triggers.triggered_rereads(move || {
        let manager = manager.clone();
        let launched = launched.clone();
        async move {
            Ok(UserServiceManager {
                state: manager.system_state().await?,
                ..launched
            })
        }
    }))
}
