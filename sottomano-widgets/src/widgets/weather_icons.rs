use iced::{Color, Element};
use sottomano_sources::weather::open_meteo_temperature::{
    Daylight::{self, Day, Night},
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
    Sky(WeatherCondition, Daylight),
    Rain,
    Thunderstorm,
    Raindrop,
    Thermometer,
    Sunrise,
    Sunset,
    StrongWind,
}

impl WeatherIcon {
    const fn svg_data(self) -> &'static [u8] {
        match self {
            WeatherIcon::Sky(condition, daylight) => sky_svg_data(condition, daylight),
            WeatherIcon::Rain => weather_svg!("wi-rain"),
            WeatherIcon::Thunderstorm => weather_svg!("wi-thunderstorm"),
            WeatherIcon::Raindrop => weather_svg!("wi-raindrop"),
            WeatherIcon::Thermometer => weather_svg!("wi-thermometer"),
            WeatherIcon::Sunrise => weather_svg!("wi-sunrise"),
            WeatherIcon::Sunset => weather_svg!("wi-sunset"),
            WeatherIcon::StrongWind => weather_svg!("wi-strong-wind"),
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self, ink_size: f32, color: Color) -> Element<'a, Message> {
        svg_icon::view(self.svg_data(), ink_size / INK_FRACTION, color)
    }
}

const fn sky_svg_data(condition: WeatherCondition, daylight: Daylight) -> &'static [u8] {
    match (condition, daylight) {
        (Clear, Day) => weather_svg!("wi-day-sunny"),
        (Clear, Night) => weather_svg!("wi-night-clear"),
        (MainlyClear, Day) => weather_svg!("wi-day-sunny-overcast"),
        (MainlyClear, Night) => weather_svg!("wi-night-alt-partly-cloudy"),
        (PartlyCloudy, Day) => weather_svg!("wi-day-cloudy"),
        (PartlyCloudy, Night) => weather_svg!("wi-night-alt-cloudy"),
        (Overcast, Day) => weather_svg!("wi-day-cloudy-high"),
        (Overcast, Night) => weather_svg!("wi-night-alt-cloudy-high"),
        (Fog | RimeFog, Day) => weather_svg!("wi-day-fog"),
        (Fog | RimeFog, Night) => weather_svg!("wi-night-fog"),
        (Drizzle(_), Day) => weather_svg!("wi-day-sprinkle"),
        (Drizzle(_), Night) => weather_svg!("wi-night-alt-sprinkle"),
        (FreezingDrizzle(_), Day) => weather_svg!("wi-day-rain-mix"),
        (FreezingDrizzle(_), Night) => weather_svg!("wi-night-alt-rain-mix"),
        (Rain(_), Day) => weather_svg!("wi-day-rain"),
        (Rain(_), Night) => weather_svg!("wi-night-alt-rain"),
        (FreezingRain(_), Day) => weather_svg!("wi-day-sleet"),
        (FreezingRain(_), Night) => weather_svg!("wi-night-alt-sleet"),
        (Snow(_) | SnowShowers(_) | SnowGrains, Day) => weather_svg!("wi-day-snow"),
        (Snow(_) | SnowShowers(_) | SnowGrains, Night) => weather_svg!("wi-night-alt-snow"),
        (RainShowers(_), Day) => weather_svg!("wi-day-showers"),
        (RainShowers(_), Night) => weather_svg!("wi-night-alt-showers"),
        (Thunderstorm, Day) => weather_svg!("wi-day-thunderstorm"),
        (Thunderstorm, Night) => weather_svg!("wi-night-alt-thunderstorm"),
        (ThunderstormWithHail(Intensity::Heavy), Day) => weather_svg!("wi-day-hail"),
        (ThunderstormWithHail(Intensity::Heavy), Night) => weather_svg!("wi-night-alt-hail"),
        (ThunderstormWithHail(_), Day) => weather_svg!("wi-day-storm-showers"),
        (ThunderstormWithHail(_), Night) => weather_svg!("wi-night-alt-storm-showers"),
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
        let sky_icons = (0..100)
            .filter_map(|code| WeatherCondition::try_from(code).ok())
            .flat_map(|condition| {
                [Day, Night].map(|daylight| WeatherIcon::Sky(condition, daylight))
            });
        let other_icons = [
            WeatherIcon::Rain,
            WeatherIcon::Thunderstorm,
            WeatherIcon::Raindrop,
            WeatherIcon::Thermometer,
            WeatherIcon::Sunrise,
            WeatherIcon::Sunset,
            WeatherIcon::StrongWind,
        ];
        sky_icons.chain(other_icons).for_each(|icon| {
            usvg::Tree::from_data(icon.svg_data(), &usvg::Options::default())
                .unwrap_or_else(|error| panic!("{icon:?}: {error}"));
        });
    }
}
