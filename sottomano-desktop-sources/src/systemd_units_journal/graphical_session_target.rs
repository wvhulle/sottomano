use std::env;

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, stream};
use tokio::task::spawn_blocking;

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    desktop_windows_apps::compositor_connection::compositor_version,
    host_machine_identity::hostname1_os_identity::at_micros,
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::{self, DesktopBuses},
    systemd_units_journal::{ManagerProxy, UnitProxy},
};

const GRAPHICAL_TARGET: &str = "graphical-session.target";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicalSession {
    pub current_desktop: Option<String>,
    pub session_desktop: Option<String>,
    pub compositor_version: String,
    pub wayland_display: String,
    pub x11_display: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicalSessionTarget {
    pub state: String,
    pub reached_at: Option<DateTime<Utc>>,
}

impl SnapshotSource for GraphicalSession {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = ();

    const NAME: &'static str = "Where apps connect to it";
    const CAPTION: SourceCaption =
        SourceCaption::Captioned("Apps find the compositor through these sockets.");

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(Self::read()).map(SnapshotDelivery::single_read)
    }
}

impl GraphicalSession {
    async fn read() -> Result<Self, dbus_desktop_buses::Error> {
        let compositor_version = spawn_blocking(compositor_version)
            .await
            .ok()
            .flatten()
            .ok_or(dbus_desktop_buses::Error::Absent)?;
        Ok(Self {
            current_desktop: env::var("XDG_CURRENT_DESKTOP").ok().and_then(non_empty),
            session_desktop: env::var("XDG_SESSION_DESKTOP").ok().and_then(non_empty),
            compositor_version,
            wayland_display: env::var("WAYLAND_DISPLAY")
                .ok()
                .and_then(non_empty)
                .ok_or(dbus_desktop_buses::Error::Absent)?,
            x11_display: env::var("DISPLAY").ok().and_then(non_empty),
        })
    }
}

impl SnapshotSource for GraphicalSessionTarget {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Desktop startup progress";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "graphical-session.target is reached once the compositor is ready for services.",
    );

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(target_changes(buses))
    }
}

async fn target_changes(
    buses: DesktopBuses,
) -> Result<
    impl Stream<Item = Result<GraphicalSessionTarget, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let manager = ManagerProxy::new(&buses.session).await?;
    manager.ensure_subscribed().await?;
    let unit = UnitProxy::builder(&buses.session)
        .path(manager.load_unit(GRAPHICAL_TARGET).await?)?
        .build()
        .await?;
    let changes = stream::select(
        unit.receive_active_state_changed().await.map(drop),
        unit.receive_active_enter_timestamp_changed()
            .await
            .map(drop),
    );
    Ok(changes.then(move |()| {
        let unit = unit.clone();
        async move {
            Ok(GraphicalSessionTarget {
                state: unit.active_state().await?,
                reached_at: at_micros(unit.active_enter_timestamp().await?),
            })
        }
    }))
}
