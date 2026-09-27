use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    sync::Arc,
    time::Duration,
};

use chrono::{NaiveDateTime, NaiveTime};
use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Deserializer, de::Error as _};
use situo_client::{Fix, Locate, fix_updates};
use zbus::Connection as SessionBus;

use crate::data::source::{Source, http};

#[derive(Debug, Clone)]
pub enum Error {
    Bus(String),
    Situo(situo_client::Error),
    Fetch(http::Error),
    Decode(Arc<serde_json::Error>),
    Malformed,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Bus(detail) => write!(f, "session bus: {detail}"),
            Error::Situo(error) => write!(f, "geolocation: {error}"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Decode(error) => write!(f, "unexpected response: {error}"),
            Error::Malformed => write!(f, "unexpected response shape"),
        }
    }
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        Error::Bus(error.to_string())
    }
}

impl From<situo_client::Error> for Error {
    fn from(error: situo_client::Error) -> Self {
        Error::Situo(error)
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Decode(Arc::new(error))
    }
}

const OPEN_METEO_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M";
const BUIENRADAR_TIME_FORMAT: &str = "%H:%M";

fn open_meteo_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveDateTime, D::Error> {
    let text = String::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&text, OPEN_METEO_TIME_FORMAT).map_err(D::Error::custom)
}

fn open_meteo_times<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<NaiveDateTime>, D::Error> {
    Vec::<String>::deserialize(deserializer)?
        .iter()
        .map(|text| NaiveDateTime::parse_from_str(text, OPEN_METEO_TIME_FORMAT))
        .collect::<Result<_, _>>()
        .map_err(D::Error::custom)
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
    time: NaiveDateTime,
    temperature_2m: f64,
    apparent_temperature: f64,
    relative_humidity_2m: f64,
    weather_code: i64,
}

#[derive(Deserialize)]
struct DailyTemperature {
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
}

#[derive(Deserialize)]
struct HourlyTemperatures {
    #[serde(deserialize_with = "open_meteo_times")]
    time: Vec<NaiveDateTime>,
    temperature_2m: Vec<f64>,
    weather_code: Vec<i64>,
}

impl HourlyTemperatures {
    fn split_at(self, now: NaiveDateTime) -> HourlyForecast {
        let (earlier, later) = self
            .time
            .into_iter()
            .zip(self.temperature_2m)
            .zip(self.weather_code)
            .map(|((time, temp), weather_code)| HourlyTemperature {
                time,
                temp,
                weather_code,
            })
            .partition(|entry| entry.time <= now);
        HourlyForecast { earlier, later }
    }
}

#[derive(Deserialize)]
struct WindForecast {
    current: CurrentWind,
}

#[derive(Deserialize)]
struct CurrentWind {
    #[serde(rename = "wind_speed_10m")]
    speed: f64,
    #[serde(rename = "wind_gusts_10m")]
    gusts: f64,
    #[serde(rename = "wind_direction_10m")]
    direction: f64,
}

#[derive(Deserialize)]
struct AirQuality {
    current: PollenReadings,
}

#[derive(Deserialize)]
pub struct PollenReadings {
    #[serde(rename = "alder_pollen")]
    alder: f64,
    #[serde(rename = "birch_pollen")]
    birch: f64,
    #[serde(rename = "grass_pollen")]
    grass: f64,
    #[serde(rename = "mugwort_pollen")]
    mugwort: f64,
    #[serde(rename = "olive_pollen")]
    olive: f64,
    #[serde(rename = "ragweed_pollen")]
    ragweed: f64,
}

impl PollenReadings {
    fn all(&self) -> Vec<(&'static str, f64)> {
        vec![
            ("Alder", self.alder),
            ("Birch", self.birch),
            ("Grass", self.grass),
            ("Mugwort", self.mugwort),
            ("Olive", self.olive),
            ("Ragweed", self.ragweed),
        ]
    }
}

fn today(values: &[f64]) -> Result<f64, Error> {
    values.first().copied().ok_or(Error::Malformed)
}

async fn located() -> Result<Fix, Error> {
    let fix = SessionBus::session().await?.locate().await?;
    if fix.valid {
        Ok(fix)
    } else {
        Err(situo_client::Error::NoFix.into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub weather_code: i64,
    pub temp: f64,
    pub feels: f64,
    pub temp_min: f64,
    pub temp_max: f64,
    pub humidity: f64,
    pub hourly: HourlyForecast,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HourlyForecast {
    pub earlier: Vec<HourlyTemperature>,
    pub later: Vec<HourlyTemperature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HourlyTemperature {
    pub time: NaiveDateTime,
    pub temp: f64,
    pub weather_code: i64,
}

impl Source for Temperature {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "weather-temp";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let geo = located().await?;
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,relative_humidity_2m,apparent_temperature&daily=temperature_2m_max,temperature_2m_min&hourly=temperature_2m,weather_code&forecast_days=2&past_hours=6&forecast_hours=12&timezone=auto",
            geo.latitude, geo.longitude
        );
        let forecast: TemperatureForecast = serde_json::from_str(&http::get_text(&url, 10).await?)?;
        Ok(Temperature {
            weather_code: forecast.current.weather_code,
            temp: forecast.current.temperature_2m,
            feels: forecast.current.apparent_temperature,
            temp_min: today(&forecast.daily.temperature_2m_min)?,
            temp_max: today(&forecast.daily.temperature_2m_max)?,
            humidity: forecast.current.relative_humidity_2m,
            hourly: forecast.hourly.split_at(forecast.current.time),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wind {
    pub speed: f64,
    pub gusts: f64,
    pub direction: f64,
}

impl Source for Wind {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "weather-wind";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let geo = located().await?;
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=wind_speed_10m,wind_gusts_10m,wind_direction_10m&timezone=auto",
            geo.latitude, geo.longitude
        );
        let forecast: WindForecast = serde_json::from_str(&http::get_text(&url, 10).await?)?;
        let CurrentWind {
            speed,
            gusts,
            direction,
        } = forecast.current;
        Ok(Wind {
            speed,
            gusts,
            direction,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rain {
    pub mm_per_h: f64,
    pub peak: Downpour,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Downpour {
    pub mm_per_h: f64,
    pub at: NaiveTime,
}

fn buienradar_mm(value: f64) -> f64 {
    if value > 0.0 {
        10f64.powf((value - 109.0) / 32.0)
    } else {
        0.0
    }
}

fn downpour(line: &str) -> Option<Downpour> {
    let (intensity, at) = line.split_once('|')?;
    Some(Downpour {
        mm_per_h: buienradar_mm(intensity.trim().parse().ok()?),
        at: NaiveTime::parse_from_str(at.trim(), BUIENRADAR_TIME_FORMAT).ok()?,
    })
}

impl Source for Rain {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "weather-rain";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let geo = located().await?;
        let url = format!(
            "https://gpsgadget.buienradar.nl/data/raintext?lat={}&lon={}",
            geo.latitude, geo.longitude
        );
        let body = http::get_text(&url, 5).await?;
        let forecast: Vec<Downpour> = body.lines().filter_map(downpour).collect();
        let peak = forecast
            .iter()
            .max_by(|earlier, later| earlier.mm_per_h.total_cmp(&later.mm_per_h))
            .cloned()
            .ok_or(Error::Malformed)?;
        Ok(Rain {
            mm_per_h: forecast.first().map_or(0.0, |now| now.mm_per_h),
            peak,
        })
    }
}

pub trait Allergen {
    const SOURCE_NAME: &'static str;
    const THRESHOLDS: [f64; 3];

    fn concentration(readings: &PollenReadings) -> f64;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grass;

impl Allergen for Grass {
    const SOURCE_NAME: &'static str = "pollen-grass";
    const THRESHOLDS: [f64; 3] = [20.0, 50.0, 80.0];

    fn concentration(readings: &PollenReadings) -> f64 {
        readings.grass
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Birch;

impl Allergen for Birch {
    const SOURCE_NAME: &'static str = "pollen-birch";
    const THRESHOLDS: [f64; 3] = [10.0, 45.0, 100.0];

    fn concentration(readings: &PollenReadings) -> f64 {
        readings.birch
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pollen<A: Allergen> {
    pub value: f64,
    pub all: Vec<(&'static str, f64)>,
    allergen: PhantomData<A>,
}

impl<A: Allergen + Clone + PartialEq + Send + Sync + 'static> Source for Pollen<A> {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = A::SOURCE_NAME;
    const INTERVAL: Duration = Duration::from_mins(30);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let geo = located().await?;
        let url = format!(
            "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto",
            geo.latitude, geo.longitude
        );
        let air: AirQuality = serde_json::from_str(&http::get_text(&url, 10).await?)?;
        Ok(Pollen {
            value: A::concentration(&air.current),
            all: air.current.all(),
            allergen: PhantomData,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub geo: Fix,
}

impl Location {
    pub fn published() -> impl Stream<Item = Result<Self, Error>> {
        fix_updates().map(|update| update.map(|geo| Self { geo }).map_err(Error::from))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const AIR_QUALITY: &str = r#"{
        "current": {
            "time": "2026-08-27T10:00", "interval": 3600,
            "alder_pollen": 0.0, "birch_pollen": 0.0, "grass_pollen": 0.3,
            "mugwort_pollen": 0.7, "olive_pollen": 0.0, "ragweed_pollen": 0.7
        }
    }"#;

    #[test]
    fn allergens_select_their_own_species() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY).unwrap();
        assert!((Grass::concentration(&air.current) - 0.3).abs() < f64::EPSILON);
        assert!((Birch::concentration(&air.current) - 0.0).abs() < f64::EPSILON);
        assert_eq!(air.current.all().len(), 6);
    }

    #[test]
    fn pollen_below_threshold_reads_zero() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY).unwrap();
        let birch: Pollen<Birch> = Pollen {
            value: Birch::concentration(&air.current),
            all: air.current.all(),
            allergen: PhantomData,
        };
        assert!(birch.value.abs() < f64::EPSILON);
    }

    #[test]
    fn hourly_temperatures_split_around_now() {
        let hourly: HourlyTemperatures = serde_json::from_str(
            r#"{
                "time": ["2026-09-27T08:00", "2026-09-27T09:00", "2026-09-27T10:00"],
                "temperature_2m": [13.2, 13.8, 15.6],
                "weather_code": [1, 3, 0]
            }"#,
        )
        .unwrap();
        let now =
            NaiveDateTime::parse_from_str("2026-09-27T09:45", OPEN_METEO_TIME_FORMAT).unwrap();
        let forecast = hourly.split_at(now);
        assert_eq!(forecast.earlier.len(), 2);
        assert_eq!(
            forecast.earlier[1].time.format("%H:%M").to_string(),
            "09:00"
        );
        assert_eq!(forecast.later.len(), 1);
        assert_eq!(forecast.later[0].weather_code, 0);
    }

    #[test]
    fn rain_lines_decode_into_downpours() {
        let dry = Rain {
            mm_per_h: 0.0,
            peak: Downpour {
                mm_per_h: 0.0,
                at: NaiveTime::from_hms_opt(9, 15, 0).unwrap(),
            },
        };
        assert!(dry.peak.mm_per_h.abs() < f64::EPSILON);
        assert_eq!(
            downpour("087|09:10").map(|rain| rain.at),
            NaiveTime::from_hms_opt(9, 10, 0)
        );
        assert!(downpour("087|9h10").is_none());
    }
}
