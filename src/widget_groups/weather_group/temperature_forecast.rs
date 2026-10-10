use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Ratio, ThermodynamicTemperature},
        ratio::percent,
        thermodynamic_temperature::degree_celsius,
    },
};

use crate::{
    data::source::weather::{HourlyTemperature, Temperature, WeatherCondition},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        tooltip_body::{Cell, KeyValue, Section, TableRecord, TooltipBody},
        weather_icons::WeatherIcon,
    },
};

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        self.condition
            .map_or_else(Self::fallback_icon, |condition| {
                WeatherIcon::from(condition).into()
            })
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::Thermometer.into()
    }

    fn label(&self) -> Option<String> {
        Some(celsius_text(self.temp))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[
            Section::untitled_column_table(
                ["", "Conditions"],
                self.condition
                    .map(|condition| TableRecord::new("Conditions", condition_cells(condition))),
            ),
            Section::dictionary(
                "Temperature",
                [
                    KeyValue::new("Now", celsius_text(self.temp)),
                    KeyValue::new("Feels like", celsius_text(self.feels)),
                    KeyValue::new("Today's low", celsius_text(self.temp_min)),
                    KeyValue::new("Today's high", celsius_text(self.temp_max)),
                    KeyValue::new("Humidity", percent_text(self.humidity)),
                ],
            ),
            Section::column_table(
                "Earlier",
                HOURLY_HEADINGS,
                self.hourly
                    .earlier
                    .iter()
                    .map(HourlyTemperature::table_record),
            ),
            Section::column_table(
                "Later",
                HOURLY_HEADINGS,
                self.hourly
                    .later
                    .iter()
                    .map(HourlyTemperature::table_record),
            ),
        ])
    }
}

const HOURLY_HEADINGS: [&str; 3] = ["Temp", "", "Conditions"];

fn condition_cells(condition: WeatherCondition) -> [Option<Cell>; 2] {
    [
        Some(Cell::Glyph(WeatherIcon::from(condition).into())),
        Some(condition.to_string().into()),
    ]
}

impl HourlyTemperature {
    fn table_record(&self) -> TableRecord<3> {
        let [glyph, text] = self.condition.map_or([None, None], condition_cells);
        TableRecord::new(
            Cell::Time {
                at: self.time,
                part: TimePart::TimeOfDay,
            },
            [Some(celsius_text(self.temp).into()), glyph, text],
        )
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
