use iced::{Color, Element};
use sottomano_sources::weather::open_meteo_temperature::{
    Intensity,
    WeatherCondition::{
        self, Clear, Drizzle, Fog, FreezingDrizzle, FreezingRain, MainlyClear, Overcast,
        PartlyCloudy, Rain, RainShowers, RimeFog, Snow, SnowGrains, SnowShowers, Thunderstorm,
        ThunderstormWithHail,
    },
};

use crate::widgets::{bar_widget::Glyph, svg_icon};

const INK_FRACTION: f32 = 22.0 / 30.0;

macro_rules! weather_svg {
    ($name:literal) => {
        include_bytes!(concat!(env!("WEATHER_ICONS_SVG"), "/", $name, ".svg"))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherIcon {
    DaySunny,
    DaySunnyOvercast,
    DayCloudy,
    Cloudy,
    Fog,
    Sprinkle,
    Rain,
    RainMix,
    Sleet,
    Showers,
    Snow,
    SnowflakeCold,
    Thunderstorm,
    StormShowers,
    Hail,
    Raindrop,
    Thermometer,
    Sunrise,
    Sunset,
    NightClear,
    StrongWind,
}

impl WeatherIcon {
    const fn svg_data(self) -> &'static [u8] {
        match self {
            WeatherIcon::Raindrop => weather_svg!("wi-raindrop"),
            WeatherIcon::Thermometer => weather_svg!("wi-thermometer"),
            WeatherIcon::Sunrise => weather_svg!("wi-sunrise"),
            WeatherIcon::Sunset => weather_svg!("wi-sunset"),
            WeatherIcon::NightClear => weather_svg!("wi-night-clear"),
            WeatherIcon::StrongWind => weather_svg!("wi-strong-wind"),
            WeatherIcon::DaySunny => weather_svg!("wi-day-sunny"),
            WeatherIcon::DaySunnyOvercast => weather_svg!("wi-day-sunny-overcast"),
            WeatherIcon::DayCloudy => weather_svg!("wi-day-cloudy"),
            WeatherIcon::Cloudy => weather_svg!("wi-cloudy"),
            WeatherIcon::Fog => weather_svg!("wi-fog"),
            WeatherIcon::Sprinkle => weather_svg!("wi-sprinkle"),
            WeatherIcon::Rain => weather_svg!("wi-rain"),
            WeatherIcon::RainMix => weather_svg!("wi-rain-mix"),
            WeatherIcon::Sleet => weather_svg!("wi-sleet"),
            WeatherIcon::Showers => weather_svg!("wi-showers"),
            WeatherIcon::Snow => weather_svg!("wi-snow"),
            WeatherIcon::SnowflakeCold => weather_svg!("wi-snowflake-cold"),
            WeatherIcon::Thunderstorm => weather_svg!("wi-thunderstorm"),
            WeatherIcon::StormShowers => weather_svg!("wi-storm-showers"),
            WeatherIcon::Hail => weather_svg!("wi-hail"),
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self, ink_size: f32, color: Color) -> Element<'a, Message> {
        svg_icon::view(self.svg_data(), ink_size / INK_FRACTION, color)
    }
}

impl From<WeatherCondition> for WeatherIcon {
    fn from(condition: WeatherCondition) -> Self {
        match condition {
            Clear => WeatherIcon::DaySunny,
            MainlyClear => WeatherIcon::DaySunnyOvercast,
            PartlyCloudy => WeatherIcon::DayCloudy,
            Overcast => WeatherIcon::Cloudy,
            Fog | RimeFog => WeatherIcon::Fog,
            Drizzle(_) => WeatherIcon::Sprinkle,
            FreezingDrizzle(_) => WeatherIcon::RainMix,
            Rain(_) => WeatherIcon::Rain,
            FreezingRain(_) => WeatherIcon::Sleet,
            Snow(_) | SnowShowers(_) => WeatherIcon::Snow,
            SnowGrains => WeatherIcon::SnowflakeCold,
            RainShowers(_) => WeatherIcon::Showers,
            Thunderstorm => WeatherIcon::Thunderstorm,
            ThunderstormWithHail(Intensity::Heavy) => WeatherIcon::Hail,
            ThunderstormWithHail(_) => WeatherIcon::StormShowers,
        }
    }
}

impl From<WeatherIcon> for Glyph {
    fn from(icon: WeatherIcon) -> Self {
        Glyph::WeatherIcon(icon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_embeds_a_parsable_svg() {
        let icons = [
            WeatherIcon::DaySunny,
            WeatherIcon::DaySunnyOvercast,
            WeatherIcon::DayCloudy,
            WeatherIcon::Cloudy,
            WeatherIcon::Fog,
            WeatherIcon::Sprinkle,
            WeatherIcon::Rain,
            WeatherIcon::RainMix,
            WeatherIcon::Sleet,
            WeatherIcon::Showers,
            WeatherIcon::Snow,
            WeatherIcon::SnowflakeCold,
            WeatherIcon::Thunderstorm,
            WeatherIcon::StormShowers,
            WeatherIcon::Hail,
            WeatherIcon::Raindrop,
            WeatherIcon::Thermometer,
            WeatherIcon::Sunrise,
            WeatherIcon::Sunset,
            WeatherIcon::NightClear,
            WeatherIcon::StrongWind,
        ];
        icons.into_iter().for_each(|icon| {
            usvg::Tree::from_data(icon.svg_data(), &usvg::Options::default())
                .unwrap_or_else(|error| panic!("{icon:?}: {error}"));
        });
    }
}
