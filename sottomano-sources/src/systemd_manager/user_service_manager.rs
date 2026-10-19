use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use zbus::proxy::CacheProperties;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    hostname::at_micros,
    pushed_delivery_stream,
    stream_operators::read_once_then_per_burst,
    system_protocol_clients::dbus_desktop_buses::{self, DesktopBuses},
    systemd_manager::{
        ManagerProxy, UNIT_INTERFACE, manager_signal, signals_matching, systemd_signal,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserServiceManager {
    pub version: String,
    pub state: String,
    pub started_at: Option<DateTime<Utc>>,
}

impl ReadingSource for UserServiceManager {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "user service manager";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(manager_state_changes(buses))
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
    let state_triggers = signals_matching(
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
    Ok(read_once_then_per_burst(state_triggers, move || {
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
