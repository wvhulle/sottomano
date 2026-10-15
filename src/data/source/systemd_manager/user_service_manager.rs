use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::{
    data::source::{
        PolledSource, Source,
        bus::{self, call},
        hostname::at_micros,
        systemd_manager::ManagerProxy,
    },
    widgets::{
        localized_time::TimePart,
        tooltip_body::{Cell, KeyValue, Section},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserServiceManager {
    pub version: String,
    pub state: String,
    pub started_at: Option<DateTime<Utc>>,
}

impl Source for UserServiceManager {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "user service manager";
}

impl PolledSource for UserServiceManager {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let connection = zbus::Connection::session()
            .await
            .map_err(|e| bus::Error::Connect(e.to_string()))?;
        let manager = call(ManagerProxy::new(&connection).await)?;
        Ok(Self {
            version: call(manager.version().await)?,
            state: call(manager.system_state().await)?,
            started_at: at_micros(call(manager.userspace_timestamp().await)?),
        })
    }
}

impl UserServiceManager {
    pub fn sections(&self) -> Vec<Section> {
        vec![
            Section::dictionary(
                [
                    Some(KeyValue::new("systemd version", self.version.clone())),
                    Some(KeyValue::new("State", self.state.clone())),
                    self.started_at.map(|at| {
                        KeyValue::new(
                            "Running since",
                            Cell::Time {
                                at,
                                part: TimePart::DateAndTime,
                            },
                        )
                    }),
                ]
                .into_iter()
                .flatten(),
            )
            .titled((
                "Your systemd instance right now",
                "One instance per user; it outlives this login and stops at your last logout.",
            )),
        ]
    }
}
