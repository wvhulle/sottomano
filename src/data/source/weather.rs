use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    time::Duration,
};

use serde_json::Value as Json;
use situo_client::{Fix, Locate, Source as FixSource};
use zbus::Connection as SessionBus;

use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value,
    font_awesome::{FontAwesome, Solid},
    http, row,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Bus(String),
    Locate(String),
    Fetch(http::Error),
    Malformed,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Bus(detail) => write!(f, "session bus: {detail}"),
            Error::Locate(detail) => write!(f, "geolocation: {detail}"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Malformed => write!(f, "unexpected response shape"),
        }
    }
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        Error::Bus(error.to_string())
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

async fn located() -> Result<Option<Fix>, Error> {
    let fix = SessionBus::session()
        .await?
        .locate()
        .await
        .map_err(|error| Error::Locate(error.to_string()))?;
    Ok(Some(fix).filter(|fix| fix.valid))
}

struct Wmo {
    icon: FontAwesome,
    description: &'static str,
}

fn wmo(code: i64) -> Wmo {
    let (icon, description) = match code {
        0 => (Solid::Sun.into(), "Clear sky"),
        1 => (Solid::Sun.into(), "Mainly clear"),
        2 => (Solid::CloudSun.into(), "Partly cloudy"),
        3 => (Solid::Cloud.into(), "Overcast"),
        45 | 48 => (Solid::Smog.into(), "Fog"),
        51 | 53 | 55 => (Solid::CloudRain.into(), "Drizzle"),
        56 | 57 => (Solid::CloudRain.into(), "Freezing drizzle"),
        61 | 63 | 65 => (Solid::CloudShowersHeavy.into(), "Rain"),
        66 | 67 => (Solid::CloudShowersHeavy.into(), "Freezing rain"),
        71 | 73 | 75 => (Solid::Snowflake.into(), "Snow"),
        77 => (Solid::Snowflake.into(), "Snow grains"),
        80..=82 => (Solid::CloudShowersHeavy.into(), "Rain showers"),
        85 | 86 => (Solid::Snowflake.into(), "Snow showers"),
        95 => (Solid::CloudBolt.into(), "Thunderstorm"),
        96 | 99 => (Solid::CloudBolt.into(), "Thunderstorm with hail"),
        _ => (Solid::CloudSun.into(), "Unknown"),
    };
    Wmo { icon, description }
}

fn round_i(json: Option<&Json>) -> Option<i64> {
    #[allow(clippy::cast_possible_truncation)]
    json.and_then(Json::as_f64).map(|v| v.round() as i64)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    icon: FontAwesome,
    conditions: &'static str,
    temp: Option<i64>,
    feels: Option<i64>,
    temp_min: Option<i64>,
    temp_max: Option<i64>,
    humidity: Option<i64>,
    wind: Option<i64>,
}

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        self.icon.into()
    }

    fn label(&self) -> Option<String> {
        Some(
            self.temp
                .map_or_else(|| "--°".to_string(), |t| format!("{t}°")),
        )
    }

    fn tooltip(&self) -> TooltipBody {
        let Some(temp) = self.temp else {
            return TooltipBody::Table(vec![row("Weather", "unavailable")]);
        };
        let mut rows = vec![
            row("Conditions", self.conditions),
            row("Temperature", format!("{temp}°C")),
        ];
        if let Some(feels) = self.feels {
            rows.push(row("Feels like", format!("{feels}°C")));
        }
        if let Some(min) = self.temp_min {
            rows.push(row("Min today", format!("{min}°C")));
        }
        if let Some(max) = self.temp_max {
            rows.push(row("Max today", format!("{max}°C")));
        }
        if let Some(humidity) = self.humidity {
            rows.push(row("Humidity", format!("{humidity}%")));
        }
        if let Some(wind) = self.wind {
            rows.push(row("Wind", format!("{wind} km/h")));
        }
        TooltipBody::Table(rows)
    }
}

impl Source for Temperature {
    type Error = Error;

    const NAME: &'static str = "weather-temp";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        let Some(geo) = located().await? else {
            return Ok(None);
        };
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code,relative_humidity_2m,wind_speed_10m,apparent_temperature&daily=temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto",
            geo.latitude, geo.longitude
        );
        let body = http::get_text(&url, 10).await?;
        let json: Json = serde_json::from_str(&body).map_err(|_| Error::Malformed)?;
        let current = json.get("current").ok_or(Error::Malformed)?;
        let daily_first = |key: &str| {
            round_i(
                json.get("daily")
                    .and_then(|daily| daily.get(key))
                    .and_then(|values| values.get(0)),
            )
        };
        let wmo = wmo(current
            .get("weather_code")
            .and_then(Json::as_i64)
            .unwrap_or(-1));
        Ok(Some(Temperature {
            icon: wmo.icon,
            conditions: wmo.description,
            temp: round_i(current.get("temperature_2m")),
            feels: round_i(current.get("apparent_temperature")),
            temp_min: daily_first("temperature_2m_min"),
            temp_max: daily_first("temperature_2m_max"),
            humidity: round_i(current.get("relative_humidity_2m")),
            wind: round_i(current.get("wind_speed_10m")),
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rain {
    icon: FontAwesome,
    mm_per_h: f64,
}

fn buienradar_mm(value: f64) -> f64 {
    if value > 0.0 {
        10f64.powf((value - 109.0) / 32.0)
    } else {
        0.0
    }
}

impl BarWidget for Rain {
    fn icon(&self) -> Glyph {
        self.icon.into()
    }

    fn tooltip(&self) -> TooltipBody {
        let value = if self.mm_per_h <= 0.0 {
            "none right now".to_string()
        } else {
            format!("{:.1} mm/h", self.mm_per_h)
        };
        TooltipBody::Table(vec![row("Rain", value)])
    }
}

impl Source for Rain {
    type Error = Error;

    const NAME: &'static str = "weather-rain";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        let Some(geo) = located().await? else {
            return Ok(None);
        };
        let url = format!(
            "https://gpsgadget.buienradar.nl/data/raintext?lat={}&lon={}",
            geo.latitude, geo.longitude
        );
        let body = http::get_text(&url, 5).await?;
        let intensities: Vec<f64> = body
            .lines()
            .take(12)
            .filter_map(|line| line.split('|').next()?.trim().parse::<f64>().ok())
            .collect();
        if intensities.is_empty() || !intensities.iter().any(|v| *v > 77.0) {
            return Ok(None);
        }
        let mm = buienradar_mm(intensities[0]);
        let icon = if mm < 0.5 {
            Solid::Droplet.into()
        } else if mm < 2.5 {
            Solid::CloudRain.into()
        } else if mm < 7.5 {
            Solid::CloudShowersHeavy.into()
        } else {
            Solid::CloudBolt.into()
        };
        Ok(Some(Rain { icon, mm_per_h: mm }))
    }
}

const SPECIES: [(&str, &str); 6] = [
    ("alder_pollen", "Alder"),
    ("birch_pollen", "Birch"),
    ("grass_pollen", "Grass"),
    ("mugwort_pollen", "Mugwort"),
    ("olive_pollen", "Olive"),
    ("ragweed_pollen", "Ragweed"),
];

pub trait Allergen {
    const SOURCE_NAME: &'static str;
    const KEY: &'static str;
    const ICON: Solid;
    const THRESHOLDS: [f64; 3];
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grass;

impl Allergen for Grass {
    const SOURCE_NAME: &'static str = "pollen-grass";
    const KEY: &'static str = "grass_pollen";
    const ICON: Solid = Solid::Seedling;
    const THRESHOLDS: [f64; 3] = [20.0, 50.0, 80.0];
}

#[derive(Debug, Clone, PartialEq)]
pub struct Birch;

impl Allergen for Birch {
    const SOURCE_NAME: &'static str = "pollen-birch";
    const KEY: &'static str = "birch_pollen";
    const ICON: Solid = Solid::Tree;
    const THRESHOLDS: [f64; 3] = [10.0, 45.0, 100.0];
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pollen<A: Allergen> {
    value: f64,
    all: Vec<(&'static str, f64)>,
    allergen: PhantomData<A>,
}

impl<A: Allergen> BarWidget for Pollen<A> {
    fn icon(&self) -> Glyph {
        A::ICON.into()
    }

    fn level(&self) -> Level {
        let [moderate, high, _] = A::THRESHOLDS;
        if self.value >= high {
            Level::Critical
        } else if self.value >= moderate {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        let cap = A::THRESHOLDS[2];
        let percent = if self.value < 1.0 {
            0
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let p = (self.value.ln() / cap.ln() * 100.0).round() as i64;
            p.clamp(0, 100)
        };
        Some(Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut all = self.all.clone();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        TooltipBody::Table(
            all.into_iter()
                .map(|(label, value)| row(label, format!("{value:.0} grains/m³")))
                .collect(),
        )
    }
}

impl<A: Allergen + Clone + PartialEq + Send + Sync + 'static> Source for Pollen<A> {
    type Error = Error;

    const NAME: &'static str = A::SOURCE_NAME;
    const INTERVAL: Duration = Duration::from_mins(30);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        let Some(geo) = located().await? else {
            return Ok(None);
        };
        let url = format!(
            "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={}&longitude={}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto",
            geo.latitude, geo.longitude
        );
        let body = http::get_text(&url, 10).await?;
        let current = serde_json::from_str::<Json>(&body)
            .map_err(|_| Error::Malformed)?
            .get("current")
            .cloned()
            .ok_or(Error::Malformed)?;
        let all: Vec<(&'static str, f64)> = SPECIES
            .iter()
            .map(|(key, label)| {
                (
                    *label,
                    current.get(*key).and_then(Json::as_f64).unwrap_or(0.0),
                )
            })
            .collect();
        let value = current.get(A::KEY).and_then(Json::as_f64).unwrap_or(0.0);
        if value <= 0.0 {
            return Ok(None);
        }
        Ok(Some(Pollen {
            value,
            all,
            allergen: PhantomData,
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    geo: Fix,
}

impl BarWidget for Location {
    fn icon(&self) -> Glyph {
        if self.geo.source == FixSource::Manual {
            Solid::LocationPinLock.into()
        } else {
            Solid::LocationDot.into()
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let dash = |s: &str| {
            if s.is_empty() {
                "—".to_string()
            } else {
                s.to_string()
            }
        };
        let distance = if self.geo.place_distance_km.is_nan() {
            "—".to_string()
        } else {
            format!("{:.2} km", self.geo.place_distance_km)
        };
        let accuracy = if self.geo.accuracy.is_nan() {
            "—".to_string()
        } else if self.geo.accuracy < 1000.0 {
            format!("{:.0} m", self.geo.accuracy)
        } else {
            format!("{:.1} km", self.geo.accuracy / 1000.0)
        };
        TooltipBody::Table(vec![
            row("Place", dash(&self.geo.place_name)),
            row("Country", dash(&self.geo.place_country)),
            row("Distance", distance),
            row("Latitude", format!("{:.4}", self.geo.latitude)),
            row("Longitude", format!("{:.4}", self.geo.longitude)),
            row("Accuracy", accuracy),
            row("Source", dash(self.geo.source.as_str())),
            row("Method", dash(&self.geo.description)),
        ])
    }
}

impl Source for Location {
    type Error = Error;

    const NAME: &'static str = "geolocation";
    const INTERVAL: Duration = Duration::from_mins(30);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        Ok(located().await?.map(|geo| Location { geo }))
    }
}
