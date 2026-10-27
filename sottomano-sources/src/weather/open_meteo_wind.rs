use std::time::Duration;

use futures::Stream;
use serde::{Deserialize, Deserializer};
use situo_client::GeoCoordinates;
use uom::si::{
    angle::degree,
    f64::{Angle, Velocity},
    velocity::kilometer_per_hour,
};

use super::Error;
use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::http_json_client,
};

const BEAUFORT_LOWER_BOUNDS_KM_PER_H: [f64; 12] = [
    1.0, 6.0, 12.0, 20.0, 29.0, 39.0, 50.0, 62.0, 75.0, 89.0, 103.0, 118.0,
];

fn kilometres_per_hour<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Velocity, D::Error> {
    Ok(Velocity::new::<kilometer_per_hour>(f64::deserialize(
        deserializer,
    )?))
}

fn compass_degrees<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Angle, D::Error> {
    Ok(Angle::new::<degree>(f64::deserialize(deserializer)?))
}

#[derive(Deserialize)]
struct WindForecast {
    current: Wind,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Wind {
    #[serde(rename = "wind_speed_10m", deserialize_with = "kilometres_per_hour")]
    pub speed: Velocity,
    #[serde(rename = "wind_gusts_10m", deserialize_with = "kilometres_per_hour")]
    pub gusts: Velocity,
    #[serde(rename = "wind_direction_10m", deserialize_with = "compass_degrees")]
    pub direction: Angle,
}

impl Wind {
    #[must_use]
    pub fn beaufort(&self) -> usize {
        BEAUFORT_LOWER_BOUNDS_KM_PER_H
            .iter()
            .take_while(|&&bound| self.speed >= Velocity::new::<kilometer_per_hour>(bound))
            .count()
    }
}

impl SnapshotSource for Wind {
    type Error = Error;
    type StreamInputs = GeoCoordinates;

    const NAME: &'static str = "weather-wind";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(300)).poll_deliveries(move || Self::poll(inputs))
    }
}

impl Wind {
    async fn poll(here: GeoCoordinates) -> Result<Self, Error> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=wind_speed_10m,wind_gusts_10m,wind_direction_10m&timezone=auto",
            here.latitude_degrees(),
            here.longitude_degrees()
        );
        let forecast: WindForecast = http_json_client::get_json(&url, &[], 10).await?;
        Ok(forecast.current)
    }
}
