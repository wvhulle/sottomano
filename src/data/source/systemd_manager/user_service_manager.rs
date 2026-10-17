use std::time::Duration;

use chrono::{DateTime, Utc};
use iced::futures::Stream;

use crate::{
    data::source::{
        DeliveryCadence, ReadingSource, SourceDelivery, bus, hostname::at_micros,
        polled_delivery_stream, systemd_manager::ManagerProxy,
    },
    widgets::{
        localized_time::LocalizedTimeFormat,
        tooltip_body::{Cell, KeyValue, Section},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserServiceManager {
    pub version: String,
    pub state: String,
    pub started_at: Option<DateTime<Utc>>,
}

impl ReadingSource for UserServiceManager {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "user service manager";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(key: Self::Key) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(key, Self::poll)
    }
}

impl UserServiceManager {
    async fn poll(_key: ()) -> Result<Self, bus::Error> {
        let connection = zbus::Connection::session()
            .await
            .map_err(|e| bus::Error::Connect(e.to_string()))?;
        let manager = ManagerProxy::new(&connection).await?;
        Ok(Self {
            version: manager.version().await?,
            state: manager.system_state().await?,
            started_at: at_micros(manager.userspace_timestamp().await?),
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
                                format: LocalizedTimeFormat::DateAndTime,
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
