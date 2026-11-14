use corredo::theming::tinted_svg_icon;
use iced::{Color, Element, Theme};
use sottomano_desktop_sources::weather::open_meteo_temperature::{
    Daylight::{self, Day, Night},
    Intensity,
    WeatherCondition::{
        self, Clear, Drizzle, Fog, FreezingDrizzle, FreezingRain, MainlyClear, Overcast,
        PartlyCloudy, Rain, RainShowers, RimeFog, Snow, SnowGrains, SnowShowers, Thunderstorm,
        ThunderstormWithHail,
    },
};
use sottomano_weather_icons::WeatherSvg;

use crate::widgets::icon_glyph::IconGlyph;

const INK_FRACTION: f32 = 22.0 / 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherIcon {
    Sky(WeatherCondition, Daylight),
    Sprinkle,
    Rain,
    Raindrops,
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
            WeatherIcon::Sprinkle => WeatherSvg::Sprinkle.svg_data(),
            WeatherIcon::Rain => WeatherSvg::Rain.svg_data(),
            WeatherIcon::Raindrops => WeatherSvg::Raindrops.svg_data(),
            WeatherIcon::Raindrop => WeatherSvg::Raindrop.svg_data(),
            WeatherIcon::Thermometer => WeatherSvg::Thermometer.svg_data(),
            WeatherIcon::Sunrise => WeatherSvg::Sunrise.svg_data(),
            WeatherIcon::Sunset => WeatherSvg::Sunset.svg_data(),
            WeatherIcon::StrongWind => WeatherSvg::StrongWind.svg_data(),
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(
        self,
        ink_size: f32,
        color: impl Fn(&Theme) -> Color + 'a,
    ) -> Element<'a, Message> {
        tinted_svg_icon(self.svg_data(), ink_size / INK_FRACTION, color)
    }
}

const fn sky_svg_data(condition: WeatherCondition, daylight: Daylight) -> &'static [u8] {
    match (condition, daylight) {
        (Clear, Day) => WeatherSvg::DaySunny.svg_data(),
        (Clear, Night) => WeatherSvg::NightClear.svg_data(),
        (MainlyClear, Day) => WeatherSvg::DaySunnyOvercast.svg_data(),
        (MainlyClear, Night) => WeatherSvg::NightAltPartlyCloudy.svg_data(),
        (PartlyCloudy, Day) => WeatherSvg::DayCloudy.svg_data(),
        (PartlyCloudy, Night) => WeatherSvg::NightAltCloudy.svg_data(),
        (Overcast, Day) => WeatherSvg::DayCloudyHigh.svg_data(),
        (Overcast, Night) => WeatherSvg::NightAltCloudyHigh.svg_data(),
        (Fog | RimeFog, Day) => WeatherSvg::DayFog.svg_data(),
        (Fog | RimeFog, Night) => WeatherSvg::NightFog.svg_data(),
        (Drizzle(_), Day) => WeatherSvg::DaySprinkle.svg_data(),
        (Drizzle(_), Night) => WeatherSvg::NightAltSprinkle.svg_data(),
        (FreezingDrizzle(_), Day) => WeatherSvg::DayRainMix.svg_data(),
        (FreezingDrizzle(_), Night) => WeatherSvg::NightAltRainMix.svg_data(),
        (Rain(_), Day) => WeatherSvg::DayRain.svg_data(),
        (Rain(_), Night) => WeatherSvg::NightAltRain.svg_data(),
        (FreezingRain(_), Day) => WeatherSvg::DaySleet.svg_data(),
        (FreezingRain(_), Night) => WeatherSvg::NightAltSleet.svg_data(),
        (Snow(_) | SnowShowers(_) | SnowGrains, Day) => WeatherSvg::DaySnow.svg_data(),
        (Snow(_) | SnowShowers(_) | SnowGrains, Night) => WeatherSvg::NightAltSnow.svg_data(),
        (RainShowers(_), Day) => WeatherSvg::DayShowers.svg_data(),
        (RainShowers(_), Night) => WeatherSvg::NightAltShowers.svg_data(),
        (Thunderstorm, Day) => WeatherSvg::DayThunderstorm.svg_data(),
        (Thunderstorm, Night) => WeatherSvg::NightAltThunderstorm.svg_data(),
        (ThunderstormWithHail(Intensity::Heavy), Day) => WeatherSvg::DayHail.svg_data(),
        (ThunderstormWithHail(Intensity::Heavy), Night) => WeatherSvg::NightAltHail.svg_data(),
        (ThunderstormWithHail(_), Day) => WeatherSvg::DayStormShowers.svg_data(),
        (ThunderstormWithHail(_), Night) => WeatherSvg::NightAltStormShowers.svg_data(),
    }
}

impl From<WeatherIcon> for IconGlyph {
    fn from(icon: WeatherIcon) -> Self {
        IconGlyph::WeatherIcon(icon)
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
            WeatherIcon::Sprinkle,
            WeatherIcon::Rain,
            WeatherIcon::Raindrops,
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
