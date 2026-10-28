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
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
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
            .map(|condition| LabeledValue::new("Sky", condition.to_string()));
        TooltipBody::sections([
            Section::dictionary(sky_entry.into_iter().chain([
                LabeledValue::new("Temperature", weather_temperature(self.temp)),
                LabeledValue::new("Feels like", weather_temperature(self.feels)),
                LabeledValue::new("Humidity", Quantity::share(self.humidity)),
            ]))
            .titled("Now"),
            Section::dictionary([
                LabeledValue::new("Low", weather_temperature(self.temp_min)),
                LabeledValue::new("High", weather_temperature(self.temp_max)),
            ])
            .titled("Today"),
            Section::column_table(HOURLY_HEADINGS, self.hourly.earlier.iter()).titled("Earlier"),
            Section::column_table(HOURLY_HEADINGS, self.hourly.later.iter())
                .marked_as_continuing()
                .titled("Later"),
        ])
    }
}
const HOURLY_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("Temp"),
    ColumnHeading::untitled(),
    ColumnHeading::titled("Sky").align_x(Horizontal::Left),
];

impl IntoTableRecord<3> for &HourlyTemperature {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(
            ValueCell::Time {
                at: self.time,
                format: LocalizedTimeFormat::HourOfDay,
            },
            [
                weather_temperature(self.temp).into(),
                self.condition.map_or(ValueCell::Blank, |condition| {
                    ValueCell::Glyph(WeatherIcon::Sky(condition, self.daylight).into())
                }),
                self.condition
                    .map_or(ValueCell::Blank, |condition| condition.to_string().into()),
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
