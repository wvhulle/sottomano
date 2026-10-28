use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_sources::weather::open_meteo_temperature::{HourlyTemperature, Temperature};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow, TooltipBody,
        },
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
                WeatherIcon::Sky(condition, self.daylight).into()
            })
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::Thermometer.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(celsius_text(self.temp))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let sky_entry = self
            .condition
            .map(|condition| KeyValue::new("Sky", condition.to_string()));
        TooltipBody::sections([
            Section::dictionary(sky_entry.into_iter().chain([
                KeyValue::new("Temperature", weather_temperature(self.temp)),
                KeyValue::new("Feels like", weather_temperature(self.feels)),
                KeyValue::new("Humidity", Quantity::share(self.humidity)),
            ]))
            .titled("Now"),
            Section::dictionary([
                KeyValue::new("Low", weather_temperature(self.temp_min)),
                KeyValue::new("High", weather_temperature(self.temp_max)),
            ])
            .titled("Today"),
            Section::column_table(HOURLY_HEADINGS, self.hourly.earlier.iter()).titled("Earlier"),
            Section::continuing_column_table(HOURLY_HEADINGS, self.hourly.later.iter())
                .titled("Later"),
        ])
    }
}
const HOURLY_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("Temp"),
    ColumnHeading::untitled(),
    ColumnHeading::titled("Sky").align_x(Horizontal::Left),
];

impl TableRow<3> for &HourlyTemperature {
    fn table_record(self) -> TableRecord<3> {
        TableRecord::new(
            Cell::Time {
                at: self.time,
                format: LocalizedTimeFormat::HourOfDay,
            },
            [
                Some(weather_temperature(self.temp).into()),
                self.condition.map(|condition| {
                    Cell::Glyph(WeatherIcon::Sky(condition, self.daylight).into())
                }),
                self.condition.map(|condition| condition.to_string().into()),
            ],
        )
    }
}

fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.1}",
        temperature.into_format_args(degree_celsius, Abbreviation)
    )
}

fn weather_temperature(temperature: ThermodynamicTemperature) -> Quantity {
    Quantity::temperature(temperature).decimals(1)
}
