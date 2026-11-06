pub mod buienradar_rain;
pub mod open_meteo_pollen;
pub mod open_meteo_precipitation_model;
pub mod open_meteo_temperature;
pub mod open_meteo_wind;
pub mod situo_location_fix;

use crate::system_protocol_clients::http_json_client;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("geolocation: {0}")]
    Situo(#[from] situo_client::ClientError),
    #[error("{0}")]
    Fetch(#[from] http_json_client::Error),
    #[error("unexpected response shape")]
    Malformed,
    #[error("rain radar only covers the Netherlands and Belgium")]
    OutsideRainRadar,
}
