use std::{env, time::Duration};

use chrono::{DateTime, Utc};
use iced::futures::Stream;
use tokio::task::spawn_blocking;

use crate::{
    data::{
        empty_text::non_empty,
        source::{
            DeliveryCadence, ReadingSource, SourceDelivery,
            bus::{self, DesktopBuses},
            hostname::{at_micros, entry},
            niri, polled_delivery_stream,
            systemd_manager::{ManagerProxy, unit_proxy},
        },
    },
    widgets::{
        localized_time::LocalizedTimeFormat,
        tooltip_body::{Cell, KeyValue, Section},
    },
};

const GRAPHICAL_TARGET: &str = "graphical-session.target";
const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";

fn unit_not_loaded(failure: &zbus::Error) -> bool {
    matches!(failure, zbus::Error::MethodError(name, _, _) if name.as_str() == NO_SUCH_UNIT)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicalSession {
    pub current_desktop: Option<String>,
    pub session_desktop: Option<String>,
    pub compositor_version: String,
    pub wayland_display: String,
    pub x11_display: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicalSessionTarget {
    NotLoaded,
    Loaded {
        state: String,
        reached_at: Option<DateTime<Utc>>,
    },
}

impl ReadingSource for GraphicalSession {
    type Error = bus::Error;
    type StreamInputs = ();

    const NAME: &'static str = "graphical session";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GraphicalSession {
    async fn poll(_key: ()) -> Result<Self, bus::Error> {
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
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl GraphicalSessionTarget {
    async fn poll(buses: DesktopBuses) -> Result<Self, bus::Error> {
        let manager = ManagerProxy::new(&buses.session).await?;
        let path = match manager.get_unit(GRAPHICAL_TARGET).await {
            Ok(path) => path,
            Err(failure) if unit_not_loaded(&failure) => return Ok(Self::NotLoaded),
            Err(failure) => return Err(failure.into()),
        };
        let unit = unit_proxy(&buses.session, path).await?;
        Ok(Self::Loaded {
            state: unit.active_state().await?,
            reached_at: at_micros(unit.active_enter_timestamp().await?),
        })
    }
}

impl GraphicalSession {
    pub fn sections(&self) -> Vec<Section> {
        let distinct_session_desktop = self
            .session_desktop
            .clone()
            .filter(|session| Some(session) != self.current_desktop.as_ref());
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("Runs", self.compositor_version.clone())),
                    Some(KeyValue::new(
                        "Wayland apps connect to",
                        self.wayland_display.clone(),
                    )),
                    entry("X11 apps connect to", self.x11_display.clone()),
                    entry("Desktop name", self.current_desktop.clone()),
                    entry("Session desktop name", distinct_session_desktop),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Where apps connect to it",
                "Apps find the compositor through these sockets.",
            )),
        ]
    }
}

impl GraphicalSessionTarget {
    pub fn sections(&self) -> Vec<Section> {
        let entries = match self {
            Self::NotLoaded => vec![KeyValue::new("State", "not loaded")],
            Self::Loaded { state, reached_at } => [
                Some(KeyValue::new("State", state.clone())),
                reached_at.map(|at| {
                    KeyValue::new(
                        "Ready since",
                        Cell::Time {
                            at,
                            format: LocalizedTimeFormat::DateAndTime,
                        },
                    )
                }),
            ]
            .into_iter()
            .flatten()
            .collect(),
        };
        vec![Section::dictionary(entries).titled((
            "Desktop startup progress",
            "graphical-session.target is reached once the compositor is ready for services.",
        ))]
    }
}
