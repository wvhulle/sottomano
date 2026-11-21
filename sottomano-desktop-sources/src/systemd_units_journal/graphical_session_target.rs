use std::env;

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, stream};
use sottomano_compositors::compositor_connection::compositor_version;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    host_machine_identity::hostname1_os_identity::at_micros,
    non_empty_text::non_empty,
    system_protocol_clients::{
        dbus_change_signals::change_triggers,
        dbus_desktop_buses::{BusReadError, DesktopBuses},
    },
    systemd_units_journal::{ManagerProxy, UNIT_INTERFACE, UnitProxy, systemd_signal},
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
    type Error = BusReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Where apps connect to it";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(Self::read()).map(SnapshotDelivery::single_read)
    }
}

impl GraphicalSession {
    async fn read() -> Result<Self, BusReadError> {
        let compositor_version = compositor_version().await.ok_or(BusReadError::Absent)?;
        Ok(Self {
            current_desktop: session_environment_variable("XDG_CURRENT_DESKTOP"),
            session_desktop: session_environment_variable("XDG_SESSION_DESKTOP"),
            compositor_version,
            wayland_display: session_environment_variable("WAYLAND_DISPLAY")
                .ok_or(BusReadError::Absent)?,
            x11_display: session_environment_variable("DISPLAY"),
        })
    }
}

fn session_environment_variable(variable_name: &str) -> Option<String> {
    match env::var(variable_name) {
        Ok(value) => non_empty(value),
        Err(env::VarError::NotPresent) => None,
        Err(unicode_failure @ env::VarError::NotUnicode(_)) => {
            tracing::warn!("graphical session: {variable_name}: {unicode_failure}");
            None
        }
    }
}

impl SnapshotSource for GraphicalSessionTarget {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Desktop startup progress";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, target_changes)
            .map(SnapshotDelivery::unavailable_when_absent)
    }
}

async fn target_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<GraphicalSessionTarget, BusReadError>> + Send, BusReadError> {
    let manager = ManagerProxy::new(&buses.session).await?;
    manager.ensure_subscribed().await?;
    let unit = UnitProxy::builder(&buses.session)
        .path(manager.load_unit(GRAPHICAL_TARGET).await?)?
        .build()
        .await?;
    let changes = change_triggers(
        &buses.session,
        [systemd_signal()?
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .arg(0, UNIT_INTERFACE)?
            .build()],
    )
    .await?;
    Ok(changes.triggered_rereads(move || {
        let unit = unit.clone();
        async move {
            Ok(GraphicalSessionTarget {
                state: unit.active_state().await?,
                reached_at: at_micros(unit.active_enter_timestamp().await?),
            })
        }
    }))
}
