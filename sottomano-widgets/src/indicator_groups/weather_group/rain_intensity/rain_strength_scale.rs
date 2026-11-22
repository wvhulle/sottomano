use std::fmt::{Display, Formatter, Result as FmtResult};

use sottomano_desktop_sources::weather::buienradar_rain::precipitation::millimeter_per_hour;
use uom::si::f64::Velocity;

use crate::glyph_icons::{
    icon_glyph::IconGlyph,
    phosphor::{Phosphor, Weight},
    weather_icons::WeatherIcon,
};

const MET_OFFICE_MODERATE_RAIN_FROM_MILLIMETERS_PER_HOUR: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RadarRainStrength {
    Dry,
    Light,
    Moderate,
    Heavy,
}

impl RadarRainStrength {
    pub(super) fn glyph(self) -> IconGlyph {
        match self {
            RadarRainStrength::Dry => Phosphor::DropSlash(Weight::Regular).into(),
            RadarRainStrength::Light => WeatherIcon::Sprinkle.into(),
            RadarRainStrength::Moderate => WeatherIcon::Rain.into(),
            RadarRainStrength::Heavy => WeatherIcon::Raindrops.into(),
        }
    }
}

impl Display for RadarRainStrength {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
            RadarRainStrength::Dry => "Dry",
            RadarRainStrength::Light => "Light",
            RadarRainStrength::Moderate => "Moderate",
            RadarRainStrength::Heavy => "Heavy",
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RainIntensityScale {
    pub(super) heavy_from: Velocity,
}

impl RainIntensityScale {
    pub(super) fn strength(self, intensity: Velocity) -> RadarRainStrength {
        if intensity <= Velocity::default() {
            RadarRainStrength::Dry
        } else if intensity
            < Velocity::new::<millimeter_per_hour>(
                MET_OFFICE_MODERATE_RAIN_FROM_MILLIMETERS_PER_HOUR,
            )
        {
            RadarRainStrength::Light
        } else if intensity < self.heavy_from {
            RadarRainStrength::Moderate
        } else {
            RadarRainStrength::Heavy
        }
    }
}
