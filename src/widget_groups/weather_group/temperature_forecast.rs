use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Ratio, ThermodynamicTemperature},
        ratio::percent,
        thermodynamic_temperature::degree_celsius,
    },
};

use crate::{
    data::source::weather::{HourlyTemperature, Temperature},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor,
        tooltip_body::{Cell, TableRow, TooltipBody, cells, row, section, table_section, untitled},
        weather_icons::WeatherIcon,
    },
};

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        WeatherIcon::from(self.condition).into()
    }

    fn fallback_icon() -> Glyph {
        phosphor::Thermometer::Regular.into()
    }

    fn label(&self) -> Option<String> {
        Some(celsius_text(self.temp))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![
            untitled(vec![cells(
                "Conditions",
                vec![
                    WeatherIcon::from(self.condition).into(),
                    self.condition.to_string().into(),
                ],
            )]),
            section(
                "Temperature",
                vec![
                    row("Now", celsius_text(self.temp)),
                    row("Feels like", celsius_text(self.feels)),
                    row("Today's low", celsius_text(self.temp_min)),
                    row("Today's high", celsius_text(self.temp_max)),
                    row("Humidity", percent_text(self.humidity)),
                ],
            ),
            table_section("Earlier", &self.hourly.earlier),
            table_section("Later", &self.hourly.later),
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
