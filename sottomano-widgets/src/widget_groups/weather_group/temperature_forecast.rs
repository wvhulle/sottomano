use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    weather::open_meteo_temperature::{HourlyTemperature, Temperature},
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table, headed_table},
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
        weather_icons::WeatherIcon,
    },
};

pub(crate) const TEMPERATURE_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::WeatherIcon(WeatherIcon::Thermometer);

pub fn temperature_reading_row(temperature: &Temperature) -> ReadingRow<'_> {
    let glyph = temperature
        .condition
        .map_or(TEMPERATURE_FALLBACK_GLYPH, |condition| {
            WeatherIcon::Sky(condition, temperature.daylight).into()
        });
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(celsius_text(temperature.temp)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            Temperature::NAME,
            glyph,
            ReadingDisplayState::Idle,
            Box::new(move |_| temperature_hover_card(temperature)),
        )
    }
}

pub fn temperature_hover_card(temperature: &Temperature) -> Vec<Section> {
    let sky_entry = temperature
        .condition
        .map(|condition| LabeledValue::new("Sky", condition.to_string()));
    vec![
        Section::new(
            "Now",
            dictionary_table(sky_entry.into_iter().chain([
                LabeledValue::new("Temperature", weather_temperature(temperature.temp)),
                LabeledValue::new("Feels like", weather_temperature(temperature.feels)),
                LabeledValue::new("Humidity", Quantity::share(temperature.humidity)),
            ])),
        ),
        Section::new(
            "Today",
            dictionary_table([
                LabeledValue::new("Low", weather_temperature(temperature.temp_min)),
                LabeledValue::new("High", weather_temperature(temperature.temp_max)),
            ]),
        ),
        Section::new(
            "Earlier",
            headed_table(HOURLY_HEADINGS, temperature.hourly.earlier.iter()),
        ),
        Section::new(
            "Later",
            headed_table(HOURLY_HEADINGS, temperature.hourly.later.iter()).marked_as_continuing(),
        ),
    ]
}

const HOURLY_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("Temp"),
    ColumnHeading::untitled(),
    ColumnHeading::titled("Sky").align_x(Horizontal::Left),
];

impl From<&HourlyTemperature> for TableRecord<3> {
    fn from(hourly_temperature: &HourlyTemperature) -> Self {
        TableRecord::with_cells(
            ValueCell::Time {
                at: hourly_temperature.time,
                format: LocalizedTimeFormat::HourOfDay,
            },
            [
                weather_temperature(hourly_temperature.temp).into(),
                hourly_temperature
                    .condition
                    .map_or(ValueCell::Blank, |condition| {
                        ValueCell::IconGlyph(
                            WeatherIcon::Sky(condition, hourly_temperature.daylight).into(),
                        )
                    }),
                hourly_temperature
                    .condition
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
