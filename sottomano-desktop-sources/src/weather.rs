pub mod buienradar_rain;
pub mod open_meteo_pollen;
pub mod open_meteo_precipitation_model;
pub mod open_meteo_temperature;
pub mod open_meteo_wind;
pub mod situo_location_fix;

use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::system_protocol_clients::http_json_client;

#[derive(Debug, Clone)]
pub enum Error {
    Situo(situo_client::ClientError),
    Fetch(http_json_client::Error),
    Malformed,
    OutsideRainRadar,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Situo(error) => write!(f, "geolocation: {error}"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Malformed => write!(f, "unexpected response shape"),
            Error::OutsideRainRadar => {
                write!(f, "rain radar only covers the Netherlands and Belgium")
            }
        }
    }
}

impl From<situo_client::ClientError> for Error {
    fn from(error: situo_client::ClientError) -> Self {
        Error::Situo(error)
    }
}

impl From<http_json_client::Error> for Error {
    fn from(error: http_json_client::Error) -> Self {
        Error::Fetch(error)
    }
}
