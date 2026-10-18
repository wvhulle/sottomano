use std::env;

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, stream};
use tokio::task::spawn_blocking;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    bus::{self, DesktopBuses},
    empty_text::non_empty,
    hostname::at_micros,
    niri, pushed_delivery_stream,
    systemd_manager::{ManagerProxy, UnitProxy},
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

impl ReadingSource for GraphicalSession {
    type Error = bus::Error;
    type StreamInputs = ();

    const NAME: &'static str = "graphical session";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(Self::read()).map(SourceDelivery::Reading)
    }
}

impl GraphicalSession {
    async fn read() -> Result<Self, bus::Error> {
        let compositor_version = spawn_blocking(niri::version)
            .await
            .ok()
            .flatten()
            .ok_or(bus::Error::Absent)?;
        Ok(Self {
            current_desktop: env::var("XDG_CURRENT_DESKTOP").ok().and_then(non_empty),
            session_desktop: env::var("XDG_SESSION_DESKTOP").ok().and_then(non_empty),
            compositor_version,
            wayland_display: env::var("WAYLAND_DISPLAY")
                .ok()
                .and_then(non_empty)
                .ok_or(bus::Error::Absent)?,
            x11_display: env::var("DISPLAY").ok().and_then(non_empty),
        })
    }
}

impl ReadingSource for GraphicalSessionTarget {
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "graphical session target";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(target_changes(buses))
    }
}

async fn target_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<GraphicalSessionTarget, bus::Error>> + Send, bus::Error> {
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
