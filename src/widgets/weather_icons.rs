use iced::{Color, Element};

use crate::{
    data::source::weather::{
        Intensity,
        WeatherCondition::{
            self, Clear, Drizzle, Fog, FreezingDrizzle, FreezingRain, MainlyClear, Overcast,
            PartlyCloudy, Rain, RainShowers, RimeFog, Snow, SnowGrains, SnowShowers, Thunderstorm,
            ThunderstormWithHail, Unrecognized,
        },
    },
    widgets::{bar_widget::Glyph, svg_icon, tooltip_body::Cell},
};

macro_rules! weather_svg {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/weather-icons/", $name, ".svg"))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    NotAvailable,
}

impl WeatherIcon {
    const fn fitted_svg(self) -> &'static [u8] {
        match self {
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
            WeatherIcon::NotAvailable => weather_svg!("wi-na"),
        }
    }

    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        svg_icon::view(self.fitted_svg(), size, color)
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
            Unrecognized(_) => WeatherIcon::NotAvailable,
        }
    }
}

impl From<WeatherIcon> for Glyph {
    fn from(icon: WeatherIcon) -> Self {
        Glyph::WeatherIcon(icon)
    }
}

impl From<WeatherIcon> for Cell {
    fn from(icon: WeatherIcon) -> Self {
        Cell::Glyph(icon.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fitted_icon_spans_the_width_and_centres_vertically() {
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
            WeatherIcon::NotAvailable,
        ];
        icons.into_iter().for_each(|icon| {
            let tree = usvg::Tree::from_data(icon.fitted_svg(), &usvg::Options::default())
                .unwrap_or_else(|error| panic!("{icon:?}: {error}"));
            let canvas = tree.size();
            let drawing = tree.root().abs_bounding_box();
            let top_margin = drawing.top();
            let bottom_margin = canvas.height() - drawing.bottom();
            assert!(
                (drawing.width() - canvas.width()).abs() < 1.0
                    && drawing.height() < canvas.height()
                    && (top_margin - bottom_margin).abs() < 1.0,
                "{icon:?}: drawing {drawing:?} is not centred across the width of canvas \
                 {canvas:?}"
            );
        });
    }
}
