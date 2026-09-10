use crate::{
    data::source::weather::{Rain, Temperature},
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        font_awesome::{FontAwesome, Solid},
        tooltip_body::{TooltipBody, row, section, untitled},
    },
};

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        wmo_icon(self.weather_code).into()
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
                ],
            ),
            section(
                "Air",
                vec![
                    row("Humidity", format!("{}%", self.humidity)),
                    row("Wind", format!("{} km/h", self.wind)),
                ],
            ),
        ])
    }
}

impl BarWidget for Rain {
    fn icon(&self) -> Glyph {
        let peak = self.peak.mm_per_h;
        if peak < 0.5 {
            Solid::Droplet.into()
        } else if peak < 2.5 {
            Solid::CloudRain.into()
        } else if peak < 7.5 {
            Solid::CloudShowersHeavy.into()
        } else {
            Solid::CloudBolt.into()
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let now = if self.mm_per_h <= 0.0 {
            "none right now".to_string()
        } else {
            format!("{:.1} mm/h", self.mm_per_h)
        };
        TooltipBody::rows(vec![
            row("Rain", now),
            row(
                "Peak",
                format!("{:.1} mm/h at {}", self.peak.mm_per_h, self.peak.at),
            ),
        ])
    }
}

impl Occasional for Rain {
    fn present(&self) -> bool {
        self.peak.mm_per_h > 0.0
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
