//! Weather sources, ported from odin's `weather-*.nu` / `pollen.nu`: current
//! temperature (Open-Meteo), current rain (Buienradar nowcast), and birch/grass
//! pollen (Open-Meteo air quality). All resolve coordinates from
//! [`Geo`](crate::data::source::geolocation::Geo) and fetch over HTTP via
//! `curl`. odin's forecast-chart popup is not ported.

use std::time::Duration;

use serde_json::Value as Json;

use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value,
    command::http_get,
    font_awesome::{FontAwesome, Solid},
    geolocation::Geo,
    row,
};

/// A WMO weather code to an icon and description (odin's `wmo-decode`).
fn wmo(code: i64) -> (FontAwesome, &'static str) {
    match code {
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
    }
}

fn round_i(json: Option<&Json>) -> Option<i64> {
    #[allow(clippy::cast_possible_truncation)]
    json.and_then(Json::as_f64).map(|v| v.round() as i64)
}

// ── current temperature (the group handle) ──────────────────────────────────

#[derive(Debug, Clone)]
pub struct Temperature {
    icon: FontAwesome,
    conditions: &'static str,
    temp: Option<i64>,
    feels: Option<i64>,
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
    const NAME: &'static str = "weather-temp";
    const INTERVAL: Duration = Duration::from_secs(300);

    fn poll() -> Option<Self> {
        let geo = Geo::resolve()?;
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?{}&current=temperature_2m,weather_code,relative_humidity_2m,wind_speed_10m,apparent_temperature&timezone=auto",
            geo.query()
        );
        // Keep the handle present (so the group shows) even when the fetch fails.
        let unavailable = Temperature {
            icon: Solid::CloudSun.into(),
            conditions: "Unknown",
            temp: None,
            feels: None,
            humidity: None,
            wind: None,
        };
        let Some(body) = http_get(&url, 10) else {
            return Some(unavailable);
        };
        let Some(current) = serde_json::from_str::<Json>(&body)
            .ok()
            .and_then(|json| json.get("current").cloned())
        else {
            return Some(unavailable);
        };
        let (icon, conditions) = wmo(current
            .get("weather_code")
            .and_then(Json::as_i64)
            .unwrap_or(-1));
        Some(Temperature {
            icon,
            conditions,
            temp: round_i(current.get("temperature_2m")),
            feels: round_i(current.get("apparent_temperature")),
            humidity: round_i(current.get("relative_humidity_2m")),
            wind: round_i(current.get("wind_speed_10m")),
        })
    }
}

// ── current rain (Buienradar nowcast) ───────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Rain {
    icon: FontAwesome,
    mm_per_h: f64,
}

/// Buienradar intensity byte (0..255) to mm/h.
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
    const NAME: &'static str = "weather-rain";
    const INTERVAL: Duration = Duration::from_secs(60);

    fn poll() -> Option<Self> {
        let geo = Geo::resolve()?;
        let url = format!(
            "https://gpsgadget.buienradar.nl/data/raintext?lat={}&lon={}",
            geo.latitude, geo.longitude
        );
        let body = http_get(&url, 5)?;
        // Each line is `value|HH:MM`; the next 60 min is the first 12 lines.
        let intensities: Vec<f64> = body
            .lines()
            .take(12)
            .filter_map(|line| line.split('|').next()?.trim().parse::<f64>().ok())
            .collect();
        // Hidden unless it is raining somewhere in the window (odin drops it).
        if intensities.is_empty() || !intensities.iter().any(|v| *v > 77.0) {
            return None;
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
        Some(Rain { icon, mm_per_h: mm })
    }
}

// ── pollen (Open-Meteo air quality) ─────────────────────────────────────────

const SPECIES: [(&str, &str); 6] = [
    ("alder_pollen", "Alder"),
    ("birch_pollen", "Birch"),
    ("grass_pollen", "Grass"),
    ("mugwort_pollen", "Mugwort"),
    ("olive_pollen", "Olive"),
    ("ragweed_pollen", "Ragweed"),
];

/// Grass vs the other tree species (birch): different icon, thresholds, and
/// cap.
#[derive(Debug, Clone)]
pub struct Pollen<const GRASS: bool> {
    value: f64,
    /// All six species' current values, for the tooltip.
    all: Vec<(&'static str, f64)>,
}

impl<const GRASS: bool> Pollen<GRASS> {
    /// `[moderate, high, very-high]` grains/m³ thresholds.
    const fn thresholds() -> [f64; 3] {
        if GRASS {
            [20.0, 50.0, 80.0]
        } else {
            [10.0, 45.0, 100.0]
        }
    }

    fn key() -> &'static str {
        if GRASS {
            "grass_pollen"
        } else {
            "birch_pollen"
        }
    }
}

impl<const GRASS: bool> BarWidget for Pollen<GRASS> {
    fn icon(&self) -> Glyph {
        if GRASS {
            Solid::Seedling.into()
        } else {
            Solid::Tree.into()
        }
    }

    fn level(&self) -> Level {
        let [moderate, high, _] = Self::thresholds();
        if self.value >= high {
            Level::Critical
        } else if self.value >= moderate {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        // Log-scaled fill between 1 grain and the very-high cap.
        let cap = Self::thresholds()[2];
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

impl<const GRASS: bool> Source for Pollen<GRASS> {
    const NAME: &'static str = if GRASS {
        "pollen-grass"
    } else {
        "pollen-birch"
    };
    const INTERVAL: Duration = Duration::from_mins(30);

    fn poll() -> Option<Self> {
        let geo = Geo::resolve()?;
        let url = format!(
            "https://air-quality-api.open-meteo.com/v1/air-quality?{}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto",
            geo.query()
        );
        let body = http_get(&url, 10)?;
        let current = serde_json::from_str::<Json>(&body)
            .ok()?
            .get("current")
            .cloned()?;
        let all: Vec<(&'static str, f64)> = SPECIES
            .iter()
            .map(|(key, label)| {
                (
                    *label,
                    current.get(*key).and_then(Json::as_f64).unwrap_or(0.0),
                )
            })
            .collect();
        let value = current
            .get(Self::key())
            .and_then(Json::as_f64)
            .unwrap_or(0.0);
        // Hidden when this species is absent (odin drops it).
        if value <= 0.0 {
            return None;
        }
        Some(Pollen { value, all })
    }
}

// ── location pin ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Location {
    geo: Geo,
}

impl BarWidget for Location {
    fn icon(&self) -> Glyph {
        if self.geo.source == "manual" {
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
        let accuracy = if self.geo.accuracy_m.is_nan() {
            "—".to_string()
        } else if self.geo.accuracy_m < 1000.0 {
            format!("{:.0} m", self.geo.accuracy_m)
        } else {
            format!("{:.1} km", self.geo.accuracy_m / 1000.0)
        };
        TooltipBody::Table(vec![
            row("Place", dash(&self.geo.place_name)),
            row("Country", dash(&self.geo.place_country)),
            row("Distance", distance),
            row("Latitude", format!("{:.4}", self.geo.latitude)),
            row("Longitude", format!("{:.4}", self.geo.longitude)),
            row("Accuracy", accuracy),
            row("Source", dash(&self.geo.source)),
            row("Method", dash(&self.geo.description)),
        ])
    }
}

impl Source for Location {
    const NAME: &'static str = "geolocation";
    const INTERVAL: Duration = Duration::from_mins(30);

    fn poll() -> Option<Self> {
        Some(Location {
            geo: Geo::resolve()?,
        })
    }
}
