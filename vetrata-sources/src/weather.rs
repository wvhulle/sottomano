use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    time::Duration,
};

use chrono::{DateTime, Local, NaiveTime, TimeDelta, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Deserializer, de::Error as DeserializeError};
use situo_client::{Fix, Position, fix_updates};
use uom::si::{
    angle::degree,
    f64::{Angle, Ratio, ThermodynamicTemperature, Velocity},
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
    velocity::kilometer_per_hour,
};

use crate::{DeliveryCadence, ReadingSource, SourceDelivery, http, polled_delivery_stream};

#[derive(Debug, Clone)]
pub enum Error {
    Situo(situo_client::Error),
    Fetch(http::Error),
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

const BUIENRADAR_TIME_FORMAT: &str = "%H:%M";
const BEAUFORT_LOWER_BOUNDS_KM_PER_H: [f64; 12] = [
    1.0, 6.0, 12.0, 20.0, 29.0, 39.0, 50.0, 62.0, 75.0, 89.0, 103.0, 118.0,
];
fn unix_instant<E: DeserializeError>(seconds: i64) -> Result<DateTime<Utc>, E> {
    DateTime::from_timestamp(seconds, 0).ok_or_else(|| E::custom("timestamp out of range"))
}

fn open_meteo_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
    unix_instant(i64::deserialize(deserializer)?)
}

fn open_meteo_times<'de, D: Deserializer<'de>>(
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

fn kilometres_per_hour<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Velocity, D::Error> {
    Ok(Velocity::new::<kilometer_per_hour>(f64::deserialize(
        deserializer,
    )?))
}

fn compass_degrees<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Angle, D::Error> {
    Ok(Angle::new::<degree>(f64::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl Display for WeatherCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
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
}

impl HourlyTemperatures {
    fn split_at(self, now: DateTime<Utc>) -> HourlyForecast {
        let (earlier, later) = self
            .time
            .into_iter()
            .zip(self.temperature_2m)
            .zip(self.weather_code)
            .map(|((time, temp), code)| HourlyTemperature {
                time,
                temp,
                condition: WeatherCondition::try_from(code).ok(),
            })
            .partition(|entry| entry.time <= now);
        HourlyForecast { earlier, later }
    }
}

#[derive(Deserialize)]
struct WindForecast {
    current: Wind,
}

#[derive(Deserialize)]
struct AirQuality {
    current: PollenReadings,
}

#[derive(Deserialize)]
pub struct PollenReadings {
    #[serde(rename = "alder_pollen")]
    alder: Option<f64>,
    #[serde(rename = "birch_pollen")]
    birch: Option<f64>,
    #[serde(rename = "grass_pollen")]
    grass: Option<f64>,
    #[serde(rename = "mugwort_pollen")]
    mugwort: Option<f64>,
    #[serde(rename = "olive_pollen")]
    olive: Option<f64>,
    #[serde(rename = "ragweed_pollen")]
    ragweed: Option<f64>,
}

impl PollenReadings {
    fn grains_per_cubic_metre(&self, allergen: Allergen) -> Option<f64> {
        match allergen {
            Allergen::Alder => self.alder,
            Allergen::Birch => self.birch,
            Allergen::Grass => self.grass,
            Allergen::Mugwort => self.mugwort,
            Allergen::Olive => self.olive,
            Allergen::Ragweed => self.ragweed,
        }
    }
}

fn today(values: &[ThermodynamicTemperature]) -> Result<ThermodynamicTemperature, Error> {
    values.first().copied().ok_or(Error::Malformed)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub condition: Option<WeatherCondition>,
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
}

impl ReadingSource for Temperature {
    type Error = Error;
    type StreamInputs = Position;

    const NAME: &'static str = "weather-temp";
    const DELIVERY_CADENCE: DeliveryCadence =
        DeliveryCadence::PolledEvery(Duration::from_secs(300));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl Temperature {
    async fn poll(here: Position) -> Result<Self, Error> {
        let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,relative_humidity_2m,apparent_temperature&daily=temperature_2m_max,temperature_2m_min&hourly=temperature_2m,weather_code&forecast_days=2&past_hours=6&forecast_hours=12&timezone=auto&timeformat=unixtime",
        here.latitude, here.longitude
    );
        let forecast: TemperatureForecast = http::get_json(&url, &[], 10).await?;
        Ok(Temperature {
            condition: WeatherCondition::try_from(forecast.current.weather_code).ok(),
            temp: forecast.current.temperature_2m,
            feels: forecast.current.apparent_temperature,
            temp_min: today(&forecast.daily.temperature_2m_min)?,
            temp_max: today(&forecast.daily.temperature_2m_max)?,
            humidity: forecast.current.relative_humidity_2m,
            hourly: forecast.hourly.split_at(forecast.current.time),
        })
    }
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

impl ReadingSource for Wind {
    type Error = Error;
    type StreamInputs = Position;

    const NAME: &'static str = "weather-wind";
    const DELIVERY_CADENCE: DeliveryCadence =
        DeliveryCadence::PolledEvery(Duration::from_secs(300));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl Wind {
    async fn poll(here: Position) -> Result<Self, Error> {
        let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=wind_speed_10m,wind_gusts_10m,wind_direction_10m&timezone=auto",
        here.latitude, here.longitude
    );
        let forecast: WindForecast = http::get_json(&url, &[], 10).await?;
        Ok(forecast.current)
    }
}

pub mod precipitation {
    use uom::{
        Conversion,
        si::{Unit, velocity},
    };

    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, Debug, Hash)]
    pub struct millimeter_per_hour;

    impl Unit for millimeter_per_hour {
        fn abbreviation() -> &'static str {
            "mm/h"
        }

        fn singular() -> &'static str {
            "millimeter per hour"
        }

        fn plural() -> &'static str {
            "millimeters per hour"
        }
    }

    impl velocity::Unit for millimeter_per_hour {}

    impl Conversion<f64> for millimeter_per_hour {
        type T = f64;

        fn coefficient() -> f64 {
            1.0 / 3_600_000.0
        }
    }

    impl velocity::Conversion<f64> for millimeter_per_hour {}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rain {
    pub intensity: Velocity,
    pub peak: Downpour,
    pub outlook: RainOutlook,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Downpour {
    pub intensity: Velocity,
    pub at: DateTime<Local>,
}

impl Downpour {
    fn is_wet(&self) -> bool {
        self.intensity > Velocity::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RainOutlook {
    Dry,
    StartsIn(TimeDelta),
    StopsIn(TimeDelta),
    OutlastsForecast(TimeDelta),
}

impl RainOutlook {
    fn of(forecast: &[Downpour]) -> Self {
        let Some((now, later)) = forecast.split_first() else {
            return RainOutlook::Dry;
        };
        if now.is_wet() {
            Self::while_raining(now, later)
        } else {
            Self::while_dry(now, later)
        }
    }

    fn while_dry(now: &Downpour, later: &[Downpour]) -> Self {
        later
            .iter()
            .find(|downpour| downpour.is_wet())
            .map_or(RainOutlook::Dry, |first_wet| {
                RainOutlook::StartsIn(first_wet.at - now.at)
            })
    }

    fn while_raining(now: &Downpour, later: &[Downpour]) -> Self {
        later
            .iter()
            .find(|downpour| !downpour.is_wet())
            .map_or_else(
                || {
                    let horizon = later.last().unwrap_or(now);
                    RainOutlook::OutlastsForecast(horizon.at - now.at)
                },
                |first_dry| RainOutlook::StopsIn(first_dry.at - now.at),
            )
    }
}

fn nearest_occurrence(clock: NaiveTime, now: DateTime<Local>) -> Option<DateTime<Local>> {
    [-1, 0, 1]
        .into_iter()
        .filter_map(|days| now.date_naive().checked_add_signed(TimeDelta::days(days)))
        .filter_map(|date| date.and_time(clock).and_local_timezone(Local).earliest())
        .min_by_key(|candidate| (*candidate - now).abs())
}

fn buienradar_intensity(value: f64) -> Velocity {
    let millimeters_per_hour = if value > 0.0 {
        10f64.powf((value - 109.0) / 32.0)
    } else {
        0.0
    };
    Velocity::new::<precipitation::millimeter_per_hour>(millimeters_per_hour)
}

fn downpour(line: &str, now: DateTime<Local>) -> Option<Downpour> {
    let (intensity, clock) = line.split_once('|')?;
    let clock = NaiveTime::parse_from_str(clock.trim(), BUIENRADAR_TIME_FORMAT).ok()?;
    Some(Downpour {
        intensity: buienradar_intensity(intensity.trim().parse().ok()?),
        at: nearest_occurrence(clock, now)?,
    })
}

impl ReadingSource for Rain {
    type Error = Error;
    type StreamInputs = Position;

    const NAME: &'static str = "weather-rain";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(60));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl Rain {
    async fn poll(here: Position) -> Result<Self, Error> {
        let url = format!(
            "https://gpsgadget.buienradar.nl/data/raintext?lat={}&lon={}",
            here.latitude, here.longitude
        );
        let body = http::get_text(&url, 5).await.map_err(|error| match error {
            http::Error::Status { status: 404, .. } => Error::OutsideRainRadar,
            other => Error::from(other),
        })?;
        let now = Local::now();
        let forecast: Vec<Downpour> = body
            .lines()
            .filter_map(|line| downpour(line, now))
            .collect();
        let peak = forecast
            .iter()
            .max_by(|earlier, later| earlier.intensity.value.total_cmp(&later.intensity.value))
            .cloned()
            .ok_or(Error::Malformed)?;
        Ok(Rain {
            intensity: forecast
                .first()
                .map_or_else(Velocity::default, |now| now.intensity),
            peak,
            outlook: RainOutlook::of(&forecast),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Allergen {
    Alder,
    Birch,
    Grass,
    Mugwort,
    Olive,
    Ragweed,
}

impl Allergen {
    pub const ALL: [Allergen; 6] = [
        Allergen::Alder,
        Allergen::Birch,
        Allergen::Grass,
        Allergen::Mugwort,
        Allergen::Olive,
        Allergen::Ragweed,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Allergen::Alder => "Alder",
            Allergen::Birch => "Birch",
            Allergen::Grass => "Grass",
            Allergen::Mugwort => "Mugwort",
            Allergen::Olive => "Olive",
            Allergen::Ragweed => "Ragweed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllergenConcentration {
    pub allergen: Allergen,
    pub grains_per_cubic_metre: f64,
}

impl AllergenConcentration {
    #[must_use]
    pub fn is_airborne(&self) -> bool {
        self.grains_per_cubic_metre > 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pollen {
    pub concentrations: Vec<AllergenConcentration>,
}

impl Pollen {
    fn of(readings: &PollenReadings) -> Self {
        Self {
            concentrations: Allergen::ALL
                .into_iter()
                .filter_map(|allergen| {
                    Some(AllergenConcentration {
                        allergen,
                        grains_per_cubic_metre: readings.grains_per_cubic_metre(allergen)?,
                    })
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn concentration(&self, allergen: Allergen) -> Option<&AllergenConcentration> {
        self.concentrations
            .iter()
            .find(|concentration| concentration.allergen == allergen)
    }

    pub fn airborne(&self) -> impl Iterator<Item = &AllergenConcentration> {
        self.concentrations
            .iter()
            .filter(|concentration| concentration.is_airborne())
    }
}

impl ReadingSource for Pollen {
    type Error = Error;
    type StreamInputs = Position;

    const NAME: &'static str = "pollen";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_mins(30));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl Pollen {
    async fn poll(here: Position) -> Result<Self, Error> {
        let url = format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto",
        here.latitude, here.longitude
    );
        let air: AirQuality = http::get_json(&url, &[], 10).await?;
        Ok(Pollen::of(&air.current))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub geo: Fix,
}

impl Location {
    #[must_use]
    pub const fn position(&self) -> Position {
        self.geo.position
    }
}

impl ReadingSource for Location {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "location";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(_key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        fix_updates()
            .map(|update| update.map(|geo| Self { geo }).map_err(Error::from))
            .map(SourceDelivery::Reading)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    const AIR_QUALITY: &str = r#"{
        "current": {
            "time": "2026-08-27T10:00", "interval": 3600,
            "alder_pollen": 0.0, "birch_pollen": 0.0, "grass_pollen": 0.3,
            "mugwort_pollen": 0.7, "olive_pollen": 0.0, "ragweed_pollen": 0.7
        }
    }"#;

    const AIR_QUALITY_OUTSIDE_EUROPE: &str = r#"{
        "current": {
            "time": "2026-10-01T02:00", "interval": 3600,
            "alder_pollen": null, "birch_pollen": null, "grass_pollen": null,
            "mugwort_pollen": null, "olive_pollen": null, "ragweed_pollen": null
        }
    }"#;

    #[test]
    fn allergens_without_model_coverage_are_left_out() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY_OUTSIDE_EUROPE).unwrap();
        let pollen = Pollen::of(&air.current);
        assert!(pollen.concentrations.is_empty());
        assert_eq!(pollen.airborne().count(), 0);
    }

    #[test]
    fn allergens_select_their_own_species() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY).unwrap();
        let pollen = Pollen::of(&air.current);
        let grains = |allergen| {
            pollen
                .concentration(allergen)
                .unwrap()
                .grains_per_cubic_metre
        };
        assert!((grains(Allergen::Grass) - 0.3).abs() < f64::EPSILON);
        assert!(grains(Allergen::Birch).abs() < f64::EPSILON);
        assert_eq!(pollen.concentrations.len(), Allergen::ALL.len());
    }

    #[test]
    fn only_allergens_with_pollen_in_the_air_are_airborne() {
        let air: AirQuality = serde_json::from_str(AIR_QUALITY).unwrap();
        let airborne: Vec<Allergen> = Pollen::of(&air.current)
            .airborne()
            .map(|concentration| concentration.allergen)
            .collect();
        assert_eq!(
            airborne,
            [Allergen::Grass, Allergen::Mugwort, Allergen::Ragweed]
        );
    }

    #[test]
    fn hourly_temperatures_split_around_now() {
        let hourly: HourlyTemperatures = serde_json::from_str(
            r#"{
                "time": [1790496000, 1790499600, 1790503200],
                "temperature_2m": [13.2, 13.8, 15.6],
                "weather_code": [1, 3, 0]
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

    fn september_27_at(hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 9, 27, hour, minute, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn rain_lines_decode_into_downpours() {
        let now = september_27_at(9, 5);
        assert_eq!(
            downpour("087|09:10", now).map(|rain| rain.at),
            Some(september_27_at(9, 10))
        );
        assert!(downpour("087|9h10", now).is_none());
    }

    #[test]
    fn clock_times_past_midnight_belong_to_the_next_day() {
        let now = september_27_at(23, 55);
        let after_midnight = downpour("000|00:05", now).unwrap().at;
        assert_eq!(after_midnight - now, TimeDelta::minutes(10));
    }

    fn rain_forecast(now: DateTime<Local>, lines: &[&str]) -> Vec<Downpour> {
        lines
            .iter()
            .filter_map(|line| downpour(line, now))
            .collect()
    }

    #[test]
    fn dry_spell_counts_down_to_the_first_rain() {
        let forecast = rain_forecast(
            september_27_at(11, 0),
            &["000|11:00", "000|11:05", "077|11:10", "000|11:15"],
        );
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::StartsIn(TimeDelta::minutes(10))
        );
    }

    #[test]
    fn ongoing_rain_counts_down_to_the_first_dry_moment() {
        let forecast = rain_forecast(
            september_27_at(23, 50),
            &["077|23:50", "077|23:55", "000|00:00"],
        );
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::StopsIn(TimeDelta::minutes(10))
        );
    }

    #[test]
    fn rain_through_the_whole_forecast_outlasts_it() {
        let forecast = rain_forecast(september_27_at(11, 0), &["077|11:00", "077|11:05"]);
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::OutlastsForecast(TimeDelta::minutes(5))
        );
    }

    #[test]
    fn no_rain_in_the_forecast_is_dry() {
        let forecast = rain_forecast(september_27_at(11, 0), &["000|11:00", "000|11:05"]);
        assert_eq!(RainOutlook::of(&forecast), RainOutlook::Dry);
    }
}
