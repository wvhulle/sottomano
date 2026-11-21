use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::Deserialize;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkyCondition {
    Recognized(WeatherCondition),
    UnrecognizedCode(i64),
}

impl From<i64> for SkyCondition {
    fn from(code: i64) -> Self {
        WeatherCondition::try_from(code).map_or(Self::UnrecognizedCode(code), Self::Recognized)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkyWetness {
    Dry,
    Precipitating,
}

impl WeatherCondition {
    #[must_use]
    pub const fn sky_wetness(self) -> SkyWetness {
        match self {
            Self::Clear
            | Self::MainlyClear
            | Self::PartlyCloudy
            | Self::Overcast
            | Self::Fog
            | Self::RimeFog => SkyWetness::Dry,
            Self::Drizzle(_)
            | Self::FreezingDrizzle(_)
            | Self::Rain(_)
            | Self::FreezingRain(_)
            | Self::Snow(_)
            | Self::SnowGrains
            | Self::RainShowers(_)
            | Self::SnowShowers(_)
            | Self::Thunderstorm
            | Self::ThunderstormWithHail(_) => SkyWetness::Precipitating,
        }
    }
}

impl Display for Intensity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
            Self::Light => "Light",
            Self::Moderate => "Moderate",
            Self::Heavy => "Heavy",
        })
    }
}

impl Display for WeatherCondition {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Clear => formatter.write_str("Clear"),
            Self::MainlyClear => formatter.write_str("Mainly clear"),
            Self::PartlyCloudy => formatter.write_str("Partly cloudy"),
            Self::Overcast => formatter.write_str("Overcast"),
            Self::Fog => formatter.write_str("Fog"),
            Self::RimeFog => formatter.write_str("Rime fog"),
            Self::Drizzle(intensity) => write!(formatter, "{intensity} drizzle"),
            Self::FreezingDrizzle(intensity) => write!(formatter, "{intensity} freezing drizzle"),
            Self::Rain(intensity) => write!(formatter, "{intensity} rain"),
            Self::FreezingRain(intensity) => write!(formatter, "{intensity} freezing rain"),
            Self::Snow(intensity) => write!(formatter, "{intensity} snow"),
            Self::SnowGrains => formatter.write_str("Snow grains"),
            Self::RainShowers(intensity) => write!(formatter, "{intensity} rain showers"),
            Self::SnowShowers(intensity) => write!(formatter, "{intensity} snow showers"),
            Self::Thunderstorm => formatter.write_str("Thunderstorm"),
            Self::ThunderstormWithHail(Intensity::Heavy) => {
                formatter.write_str("Thunderstorm with heavy hail")
            }
            Self::ThunderstormWithHail(_) => formatter.write_str("Thunderstorm with hail"),
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
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
