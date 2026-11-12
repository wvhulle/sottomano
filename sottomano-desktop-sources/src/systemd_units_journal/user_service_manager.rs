use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use zbus::proxy::CacheProperties;

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    delivery_cadence_drivers::ChangeTriggerStreams,
    host_machine_identity::hostname1_os_identity::at_micros,
    system_protocol_clients::{
        dbus_change_signals::change_triggers,
        dbus_desktop_buses::{self, DesktopBuses},
    },
    systemd_units_journal::{ManagerProxy, UNIT_INTERFACE, manager_signal, systemd_signal},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserServiceManager {
    pub version: String,
    pub state: String,
    pub started_at: Option<DateTime<Utc>>,
}

impl SnapshotSource for UserServiceManager {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Your systemd instance right now";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "One instance per user; it outlives this login and stops at your last logout.",
    );

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, manager_state_changes)
    }
}

async fn manager_state_changes(
    buses: DesktopBuses,
) -> Result<
    impl Stream<Item = Result<UserServiceManager, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let manager = ManagerProxy::builder(&buses.session)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    manager.ensure_subscribed().await?;
    let launched = UserServiceManager {
        version: manager.version().await?,
        state: manager.system_state().await?,
        started_at: at_micros(manager.userspace_timestamp().await?),
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
    .await?
    .map(drop);
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
