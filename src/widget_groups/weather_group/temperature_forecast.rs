use crate::{
    data::source::weather::{HourlyTemperature, Temperature},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::{FontAwesome, Solid},
        tooltip_body::{Row, TooltipBody, row, section, untitled},
    },
};

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        wmo_icon(self.weather_code).into()
    }

    fn fallback_icon() -> Glyph {
        Solid::Thermometer.into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{}°", self.temp))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![
            untitled(vec![row("Conditions", wmo_description(self.weather_code))]),
            section(
                "Temperature",
                vec![
                    row("Now", format!("{}°C (feels {}°C)", self.temp, self.feels)),
                    row(
                        "Today",
                        format!("{}°C – {}°C", self.temp_min, self.temp_max),
                    ),
                    row("Humidity", format!("{}%", self.humidity)),
                ],
            ),
            section(
                "Earlier",
                self.hourly
                    .earlier
                    .iter()
                    .map(HourlyTemperature::row)
                    .collect(),
            ),
            section(
                "Later",
                self.hourly
                    .later
                    .iter()
                    .map(HourlyTemperature::row)
                    .collect(),
            ),
        ])
    }
}

impl HourlyTemperature {
    fn row(&self) -> Row {
        row(
            &self.time.format("%H:%M").to_string(),
            format!("{}°C  {}", self.temp, wmo_description(self.weather_code)),
        )
    }
}

fn wmo_icon(code: i64) -> FontAwesome {
    match code {
        0 | 1 => Solid::Sun.into(),
        3 => Solid::Cloud.into(),
        45 | 48 => Solid::Smog.into(),
        51 | 53 | 55 | 56 | 57 => Solid::CloudRain.into(),
        61 | 63 | 65 | 66 | 67 | 80..=82 => Solid::CloudShowersHeavy.into(),
        71 | 73 | 75 | 77 | 85 | 86 => Solid::Snowflake.into(),
        95 | 96 | 99 => Solid::CloudBolt.into(),
        _ => Solid::CloudSun.into(),
    }
}

fn wmo_description(code: i64) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 => "Snow",
        77 => "Snow grains",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}
