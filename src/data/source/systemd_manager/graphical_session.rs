use std::{env, time::Duration};

use chrono::{DateTime, Utc};
use tokio::task::spawn_blocking;

use crate::{
    data::{
        empty_text::non_empty,
        source::{
            PolledSource, Source,
            bus::{self, call},
            hostname::{at_micros, entry},
            niri,
            systemd_manager::{ManagerProxy, unit_proxy},
        },
    },
    widgets::{
        localized_time::TimePart,
        tooltip_body::{Cell, KeyValue, Section},
    },
};

const GRAPHICAL_TARGET: &str = "graphical-session.target";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicalTarget {
    pub state: String,
    pub reached_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicalSession {
    pub current_desktop: Option<String>,
    pub session_desktop: Option<String>,
    pub compositor_version: String,
    pub display: String,
    pub target: Option<GraphicalTarget>,
}

impl Source for GraphicalSession {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "graphical session";
}

impl PolledSource for GraphicalSession {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let connection = zbus::Connection::session()
            .await
            .map_err(|e| bus::Error::Connect(e.to_string()))?;
        let manager = call(ManagerProxy::new(&connection).await)?;
        let target = match manager.get_unit(GRAPHICAL_TARGET).await {
            Ok(path) => {
                let unit = call(unit_proxy(&connection, path).await)?;
                Some(GraphicalTarget {
                    state: call(unit.active_state().await)?,
                    reached_at: at_micros(call(unit.active_enter_timestamp().await)?),
                })
            }
            Err(_) => None,
        };
        let compositor_version = spawn_blocking(niri::version)
            .await
            .ok()
            .flatten()
            .ok_or(bus::Error::Absent)?;
        Ok(Self {
            current_desktop: env::var("XDG_CURRENT_DESKTOP").ok().and_then(non_empty),
            session_desktop: env::var("XDG_SESSION_DESKTOP").ok().and_then(non_empty),
            compositor_version,
            display: env::var("WAYLAND_DISPLAY")
                .ok()
                .and_then(non_empty)
                .ok_or(bus::Error::Absent)?,
            target,
        })
    }
}

impl GraphicalSession {
    pub fn sections(&self) -> Vec<Section> {
        let target = self.target.as_ref().map_or_else(
            || vec![KeyValue::new(GRAPHICAL_TARGET, "not loaded")],
            |target| {
                [
                    Some(KeyValue::new(GRAPHICAL_TARGET, target.state.clone())),
                    target.reached_at.map(|at| {
                        KeyValue::new(
                            "Reached",
                            Cell::Time {
                                at,
                                part: TimePart::DateAndTime,
                            },
                        )
                    }),
                ]
                .into_iter()
                .flatten()
                .collect()
            },
        );
        vec![
            Section::dictionary(
                [
                    entry("Desktop", self.current_desktop.clone()),
                    entry("Session desktop", self.session_desktop.clone()),
                    Some(KeyValue::new("Compositor", self.compositor_version.clone())),
                    Some(KeyValue::new("Display socket", self.display.clone())),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Display server"),
            Section::dictionary(
                [KeyValue::new("Session manager", "systemd --user")]
                    .into_iter()
                    .chain(target),
            )
            .titled("Session manager"),
        ]
    }
}
