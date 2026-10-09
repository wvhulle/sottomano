use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Ratio, ThermodynamicTemperature},
        ratio::percent,
        thermodynamic_temperature::degree_celsius,
    },
};

use crate::{
    data::{
        severity::Severity,
        source::weather::{HourlyTemperature, Temperature},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        tooltip_body::{Cell, Row, Section, TableRow, TooltipBody},
        weather_icons::WeatherIcon,
    },
};

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        WeatherIcon::from(self.condition).into()
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::Thermometer.into()
    }

    fn label(&self) -> Option<String> {
        Some(celsius_text(self.temp))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[
            Section::untitled(&[Row::new(
                "Conditions",
                &[
                    WeatherIcon::from(self.condition).into(),
                    self.condition.to_string().into(),
                ],
                Severity::Normal,
            )]),
            Section::titled(
                "Temperature",
                &[
                    ("Now", celsius_text(self.temp)),
                    ("Feels like", celsius_text(self.feels)),
                    ("Today's low", celsius_text(self.temp_min)),
                    ("Today's high", celsius_text(self.temp_max)),
                    ("Humidity", percent_text(self.humidity)),
                ],
            ),
            Section::table("Earlier", &self.hourly.earlier),
            Section::table("Later", &self.hourly.later),
        ])
    }
}

impl TableRow for HourlyTemperature {
    const HEADINGS: &'static [&'static str] = &["Temp", "", "Conditions"];

    fn key(&self) -> Cell {
        Cell::Time {
            at: self.time,
            part: TimePart::TimeOfDay,
        }
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            celsius_text(self.temp).into(),
            WeatherIcon::from(self.condition).into(),
            self.condition.to_string().into(),
        ]
    }
}

fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.1}",
        temperature.into_format_args(degree_celsius, Abbreviation)
    )
}

fn percent_text(fraction: Ratio) -> String {
    format!("{:.0}", fraction.into_format_args(percent, Abbreviation))
}
