use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    weather::open_meteo_temperature::{HourlyTemperature, SkyCondition, Temperature},
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use super::{WEATHER_GLYPH, WeatherGroup, WeatherSource, refresh_online};
use crate::{
    glyph_icons::{icon_glyph::IconGlyph, weather_icons::WeatherIcon},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(crate) const TEMPERATURE_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::WeatherIcon(WeatherIcon::Thermometer);

pub fn temperature_indicator(temperature: &Temperature) -> GroupIndicator<'_> {
    let glyph = match temperature.condition {
        SkyCondition::Recognized(condition) => {
            WeatherIcon::Sky(condition, temperature.daylight).into()
        }
        SkyCondition::UnrecognizedCode(_) => TEMPERATURE_FALLBACK_GLYPH,
    };
    GroupIndicator::glyph_only(Temperature::NAME, glyph, IndicatorState::Idle)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(celsius_text(temperature.temp)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| temperature_hover_card(temperature)))
}

pub fn temperature_hover_card(temperature: &Temperature) -> Vec<Section> {
    let mut now_entries = Vec::new();
    if let SkyCondition::Recognized(condition) = temperature.condition {
        now_entries.push(LabeledValue::new("Sky", condition.to_string()));
    }
    now_entries.extend([
        LabeledValue::new("Temperature", weather_temperature(temperature.temp)),
        LabeledValue::new("Feels like", weather_temperature(temperature.feels)),
        LabeledValue::new("Humidity", Quantity::share(temperature.humidity)),
    ]);
    vec![
        Section::new("Now", dictionary_table(now_entries)),
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
        let (sky_icon_cell, sky_text_cell) = match hourly_temperature.condition {
            SkyCondition::Recognized(condition) => (
                ValueCell::IconGlyph(
                    WeatherIcon::Sky(condition, hourly_temperature.daylight).into(),
                ),
                condition.to_string().into(),
            ),
            SkyCondition::UnrecognizedCode(_) => (ValueCell::Blank, ValueCell::Blank),
        };
        TableRecord::with_cells(
            ValueCell::Time {
                at: hourly_temperature.time,
                format: LocalizedTimeFormat::HourOfDay,
            },
            [
                weather_temperature(hourly_temperature.temp).into(),
                sky_icon_cell,
                sky_text_cell,
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

impl WeatherGroup {
    pub(super) fn temperature_listing(&self) -> IndicatorListing<'_> {
        self.temperature
            .indicator_listing(
                |held_snapshot| vec![temperature_indicator(held_snapshot)],
                WEATHER_GLYPH.into(),
            )
            .map_indicators(|temperature_row| self.wired_temperature_indicator(temperature_row))
    }

    pub(super) fn wired_temperature_indicator<'view>(
        &'view self,
        temperature_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        temperature_row
            .with_hover_card(Box::new(move |_| {
                self.temperature
                    .sections_described_by(temperature_hover_card)
            }))
            .refreshed_by(refresh_online(WeatherSource::Temperature))
    }
}
