use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use chrono::{DateTime, Utc};
use futures::Stream;
use serde::{Deserialize, Deserializer, de::Error as DeserializeError};
use situo_client::GeoCoordinates;
use uom::si::{
    f64::{Ratio, ThermodynamicTemperature},
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use super::Error;
use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::http_json_client,
};

fn unix_instant<E: DeserializeError>(seconds: i64) -> Result<DateTime<Utc>, E> {
    DateTime::from_timestamp(seconds, 0).ok_or_else(|| E::custom("timestamp out of range"))
}

pub(super) fn open_meteo_time<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<DateTime<Utc>, D::Error> {
    unix_instant(i64::deserialize(deserializer)?)
}

pub(super) fn open_meteo_times<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<DateTime<Utc>>, D::Error> {
    Vec::<i64>::deserialize(deserializer)?
        .into_iter()
        .map(unix_instant)
        .collect()
}

fn celsius<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<ThermodynamicTemperature, D::Error> {
    Ok(ThermodynamicTemperature::new::<degree_celsius>(
        f64::deserialize(deserializer)?,
    ))
}

fn celsius_series<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ThermodynamicTemperature>, D::Error> {
    Ok(Vec::<f64>::deserialize(deserializer)?
        .into_iter()
        .map(ThermodynamicTemperature::new::<degree_celsius>)
        .collect())
}

fn percent_value<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Ratio, D::Error> {
    Ok(Ratio::new::<percent>(f64::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intensity {
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "i64")]
pub enum WeatherCondition {
    Clear,
    MainlyClear,
    PartlyCloudy,
    Overcast,
    Fog,
    RimeFog,
    Drizzle(Intensity),
    FreezingDrizzle(Intensity),
    Rain(Intensity),
    FreezingRain(Intensity),
    Snow(Intensity),
    SnowGrains,
    RainShowers(Intensity),
    SnowShowers(Intensity),
    Thunderstorm,
    ThunderstormWithHail(Intensity),
}

impl Display for Intensity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            Self::Light => "Light",
            Self::Moderate => "Moderate",
            Self::Heavy => "Heavy",
        })
    }
}

impl Display for WeatherCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Clear => f.write_str("Clear"),
            Self::MainlyClear => f.write_str("Mainly clear"),
            Self::PartlyCloudy => f.write_str("Partly cloudy"),
            Self::Overcast => f.write_str("Overcast"),
            Self::Fog => f.write_str("Fog"),
            Self::RimeFog => f.write_str("Rime fog"),
            Self::Drizzle(intensity) => write!(f, "{intensity} drizzle"),
            Self::FreezingDrizzle(intensity) => write!(f, "{intensity} freezing drizzle"),
            Self::Rain(intensity) => write!(f, "{intensity} rain"),
            Self::FreezingRain(intensity) => write!(f, "{intensity} freezing rain"),
            Self::Snow(intensity) => write!(f, "{intensity} snow"),
            Self::SnowGrains => f.write_str("Snow grains"),
            Self::RainShowers(intensity) => write!(f, "{intensity} rain showers"),
            Self::SnowShowers(intensity) => write!(f, "{intensity} snow showers"),
            Self::Thunderstorm => f.write_str("Thunderstorm"),
            Self::ThunderstormWithHail(Intensity::Heavy) => {
                f.write_str("Thunderstorm with heavy hail")
            }
            Self::ThunderstormWithHail(_) => f.write_str("Thunderstorm with hail"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(from = "u8")]
pub enum Daylight {
    Day,
    Night,
}

impl Display for Daylight {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            Self::Day => "Day",
            Self::Night => "Night",
        })
    }
}

impl From<u8> for Daylight {
    fn from(is_day: u8) -> Self {
        if is_day == 0 { Self::Night } else { Self::Day }
    }
}

impl TryFrom<i64> for WeatherCondition {
    type Error = i64;

    fn try_from(code: i64) -> Result<Self, Self::Error> {
        use Intensity::{Heavy, Light, Moderate};
        let condition = match code {
            0 => Self::Clear,
            1 => Self::MainlyClear,
            2 => Self::PartlyCloudy,
            3 => Self::Overcast,
            45 => Self::Fog,
            48 => Self::RimeFog,
            51 => Self::Drizzle(Light),
            53 => Self::Drizzle(Moderate),
            55 => Self::Drizzle(Heavy),
            56 => Self::FreezingDrizzle(Light),
            57 => Self::FreezingDrizzle(Heavy),
            61 => Self::Rain(Light),
            63 => Self::Rain(Moderate),
            65 => Self::Rain(Heavy),
            66 => Self::FreezingRain(Light),
            67 => Self::FreezingRain(Heavy),
            71 => Self::Snow(Light),
            73 => Self::Snow(Moderate),
            75 => Self::Snow(Heavy),
            77 => Self::SnowGrains,
            80 => Self::RainShowers(Light),
            81 => Self::RainShowers(Moderate),
            82 => Self::RainShowers(Heavy),
            85 => Self::SnowShowers(Light),
            86 => Self::SnowShowers(Heavy),
            95 => Self::Thunderstorm,
            96 => Self::ThunderstormWithHail(Light),
            99 => Self::ThunderstormWithHail(Heavy),
            unrecognized => return Err(unrecognized),
        };
        Ok(condition)
    }
}

#[derive(Deserialize)]
struct TemperatureForecast {
    current: CurrentTemperature,
    daily: DailyTemperature,
    hourly: HourlyTemperatures,
}

#[derive(Deserialize)]
struct CurrentTemperature {
    #[serde(deserialize_with = "open_meteo_time")]
    time: DateTime<Utc>,
    #[serde(deserialize_with = "celsius")]
    temperature_2m: ThermodynamicTemperature,
    #[serde(deserialize_with = "celsius")]
    apparent_temperature: ThermodynamicTemperature,
    #[serde(deserialize_with = "percent_value")]
    relative_humidity_2m: Ratio,
    weather_code: i64,
    is_day: Daylight,
}

#[derive(Deserialize)]
struct DailyTemperature {
    #[serde(deserialize_with = "celsius_series")]
    temperature_2m_max: Vec<ThermodynamicTemperature>,
    #[serde(deserialize_with = "celsius_series")]
    temperature_2m_min: Vec<ThermodynamicTemperature>,
}

#[derive(Deserialize)]
struct HourlyTemperatures {
    #[serde(deserialize_with = "open_meteo_times")]
    time: Vec<DateTime<Utc>>,
    #[serde(deserialize_with = "celsius_series")]
    temperature_2m: Vec<ThermodynamicTemperature>,
    weather_code: Vec<i64>,
    is_day: Vec<Daylight>,
}

impl HourlyTemperatures {
    fn split_at(self, now: DateTime<Utc>) -> HourlyForecast {
        let (earlier, later) = self
            .time
            .into_iter()
            .zip(self.temperature_2m)
            .zip(self.weather_code)
            .zip(self.is_day)
            .map(|(((time, temp), code), daylight)| HourlyTemperature {
                time,
                temp,
                condition: WeatherCondition::try_from(code).ok(),
                daylight,
            })
            .partition(|entry| entry.time <= now);
        HourlyForecast { earlier, later }
    }
}

fn today(values: &[ThermodynamicTemperature]) -> Result<ThermodynamicTemperature, Error> {
    values.first().copied().ok_or(Error::Malformed)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub condition: Option<WeatherCondition>,
    pub daylight: Daylight,
    pub temp: ThermodynamicTemperature,
    pub feels: ThermodynamicTemperature,
    pub temp_min: ThermodynamicTemperature,
    pub temp_max: ThermodynamicTemperature,
    pub humidity: Ratio,
    pub hourly: HourlyForecast,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HourlyForecast {
    pub earlier: Vec<HourlyTemperature>,
    pub later: Vec<HourlyTemperature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HourlyTemperature {
    pub time: DateTime<Utc>,
    pub temp: ThermodynamicTemperature,
    pub condition: Option<WeatherCondition>,
    pub daylight: Daylight,
}

impl SnapshotSource for Temperature {
    type Error = Error;
    type StreamInputs = GeoCoordinates;

    const NAME: &'static str = "Weather temperature";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(300)).poll_deliveries(move || Self::poll(inputs))
    }
}

impl Temperature {
    async fn poll(here: GeoCoordinates) -> Result<Self, Error> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,is_day,relative_humidity_2m,apparent_temperature&daily=temperature_2m_max,temperature_2m_min&hourly=temperature_2m,weather_code,is_day&forecast_days=2&past_hours=6&forecast_hours=12&timezone=auto&timeformat=unixtime",
            here.latitude_degrees(),
            here.longitude_degrees()
        );
        let forecast: TemperatureForecast = http_json_client::get_json(&url, &[], 10).await?;
        Ok(Temperature {
            condition: WeatherCondition::try_from(forecast.current.weather_code).ok(),
            daylight: forecast.current.is_day,
            temp: forecast.current.temperature_2m,
            feels: forecast.current.apparent_temperature,
            temp_min: today(&forecast.daily.temperature_2m_min)?,
            temp_max: today(&forecast.daily.temperature_2m_max)?,
            humidity: forecast.current.relative_humidity_2m,
            hourly: forecast.hourly.split_at(forecast.current.time),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hourly_temperatures_split_around_now() {
        let hourly: HourlyTemperatures = serde_json::from_str(
            r#"{
                "time": [1790496000, 1790499600, 1790503200],
                "temperature_2m": [13.2, 13.8, 15.6],
                "weather_code": [1, 3, 0],
                "is_day": [0, 1, 1]
            }"#,
        )
        .unwrap();
        let now = "2026-09-27T09:45:00Z".parse().unwrap();
        let forecast = hourly.split_at(now);
        assert_eq!(forecast.earlier.len(), 2);
        assert_eq!(
            forecast.earlier[1].time,
            "2026-09-27T09:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(forecast.later.len(), 1);
        assert_eq!(forecast.later[0].condition, Some(WeatherCondition::Clear));
        assert_eq!(forecast.earlier[0].daylight, Daylight::Night);
        assert_eq!(forecast.later[0].daylight, Daylight::Day);
    }

    #[test]
    fn conditions_read_as_english() {
        assert_eq!(WeatherCondition::PartlyCloudy.to_string(), "Partly cloudy");
        assert_eq!(
            WeatherCondition::RainShowers(Intensity::Moderate).to_string(),
            "Moderate rain showers"
        );
        assert_eq!(
            WeatherCondition::ThunderstormWithHail(Intensity::Heavy).to_string(),
            "Thunderstorm with heavy hail"
        );
    }

    #[test]
    fn documented_codes_decode_into_recognized_conditions() {
        let documented = [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99,
        ];
        assert!(
            documented
                .into_iter()
                .all(|code| WeatherCondition::try_from(code).is_ok())
        );
        assert_eq!(
            WeatherCondition::try_from(57),
            Ok(WeatherCondition::FreezingDrizzle(Intensity::Heavy))
        );
        assert_eq!(WeatherCondition::try_from(42), Err(42));
    }
}
