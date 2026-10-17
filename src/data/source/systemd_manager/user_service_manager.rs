use std::time::Duration;

use chrono::{DateTime, Utc};
use iced::futures::Stream;

use crate::{
    data::source::{
        DeliveryCadence, ReadingSource, SourceDelivery,
        bus::{self, DesktopBuses},
        hostname::at_micros,
        polled_delivery_stream,
        systemd_manager::ManagerProxy,
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
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "user service manager";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl UserServiceManager {
    async fn poll(buses: DesktopBuses) -> Result<Self, bus::Error> {
        let manager = ManagerProxy::new(&buses.session).await?;
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
