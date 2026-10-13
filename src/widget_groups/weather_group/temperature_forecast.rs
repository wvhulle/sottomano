use iced::alignment::Horizontal;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use crate::{
    data::source::weather::{HourlyTemperature, Temperature, WeatherCondition},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
        localized_time::TimePart,
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
        weather_icons::WeatherIcon,
    },
};

impl BarWidget for Temperature {
    fn name(&self) -> &'static str {
        "Temperature forecast"
    }

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
        TooltipBody::sections([
            Section::column_table(
                [
                    ColumnHeading::new(""),
                    ColumnHeading::new("Conditions").align_x(Horizontal::Left),
                ],
                self.condition
                    .map(|condition| TableRecord::new("Conditions", condition_cells(condition))),
            ),
            Section::dictionary([
                KeyValue::new("Now", celsius_text(self.temp)),
                KeyValue::new("Feels like", celsius_text(self.feels)),
                KeyValue::new("Today's low", celsius_text(self.temp_min)),
                KeyValue::new("Today's high", celsius_text(self.temp_max)),
                KeyValue::new("Humidity", percent_text(self.humidity)),
            ])
            .titled("Temperature"),
            Section::column_table(
                HOURLY_HEADINGS,
                self.hourly
                    .earlier
                    .iter()
                    .map(HourlyTemperature::table_record),
            )
            .titled("Earlier"),
            Section::column_table(
                HOURLY_HEADINGS,
                self.hourly
                    .later
                    .iter()
                    .map(HourlyTemperature::table_record),
            )
            .titled("Later"),
        ])
    }
}

const HOURLY_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::new("Temp"),
    ColumnHeading::new(""),
    ColumnHeading::new("Conditions").align_x(Horizontal::Left),
];

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
