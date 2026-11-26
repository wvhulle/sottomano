use chrono::{DateTime, Local, NaiveDate, Timelike, Utc};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::weather::{
    open_meteo_precipitation_model::ModelQuarterHour, open_meteo_temperature::SkyWetness,
};
use uom::si::f64::Velocity;

use super::rain_strength_scale::{RadarRainStrength, RainIntensityScale};
use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    hover_cards::hover_card_body::{Section, headed_grouped_table},
    panel_menus::agenda_menu::weather_group::rain_indicator::{
        precipitation_type_forecast::{
            MODEL_QUARTER_HOUR, model_quarter_glyph, model_quarter_text,
        },
        rain_forecast_ranking::{RainRateIndicator, RankedRateSource, rate_indicators_by_rank},
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        value_cells::ValueCell,
    },
};

const TIMELINE_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("Rain"),
    ColumnHeading::untitled(),
    ColumnHeading::titled("Type").align_x(Horizontal::Left),
];

#[derive(Debug, Clone, Copy)]
pub(super) enum PrecipitationTypeMarker<'forecast> {
    Unmarked,
    StartsQuarter(&'forecast ModelQuarterHour),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RainTimelineRow<'forecast> {
    pub(super) sample: RainRateIndicator<'forecast>,
    pub(super) type_marker: PrecipitationTypeMarker<'forecast>,
    pub(super) scale: RainIntensityScale,
}

impl RainTimelineRow<'_> {
    fn starting_hour(self) -> TimelineHour {
        let starts_at = self.sample.starts_at().with_timezone(&Local);
        TimelineHour {
            date: starts_at.date_naive(),
            hour: starts_at.hour(),
        }
    }

    fn glyph(self) -> IconGlyph {
        let strength_glyph = self.scale.strength(self.sample.intensity()).glyph();
        match self.sample {
            RainRateIndicator::RadarFrame(_) => strength_glyph,
            RainRateIndicator::ModelQuarter(quarter) => {
                model_quarter_glyph(quarter, strength_glyph)
            }
        }
    }

    fn precipitation_type_cell(self) -> ValueCell {
        match (self.sample, self.type_marker) {
            (RainRateIndicator::ModelQuarter(quarter), _) => model_quarter_text(quarter).into(),
            (sample, PrecipitationTypeMarker::StartsQuarter(quarter))
                if sample.sky_wetness() == SkyWetness::Precipitating
                    && quarter.condition.sky_wetness() == SkyWetness::Precipitating =>
            {
                model_quarter_text(quarter).into()
            }
            (RainRateIndicator::RadarFrame(_), _) => ValueCell::Blank,
        }
    }
}

fn rain_amount_cell(intensity: Velocity, scale: RainIntensityScale) -> ValueCell {
    match scale.strength(intensity) {
        RadarRainStrength::Dry => ValueCell::from("–"),
        _ => Quantity::precipitation_rate(intensity).into(),
    }
}

impl From<&RainTimelineRow<'_>> for TableRecord<3> {
    fn from(rain_timeline_row: &RainTimelineRow<'_>) -> Self {
        let time = ValueCell::Time {
            at: rain_timeline_row.sample.starts_at(),
            format: LocalizedTimeFormat::HourAndMinute,
        };
        TableRecord::with_cells(
            time,
            [
                rain_amount_cell(
                    rain_timeline_row.sample.intensity(),
                    rain_timeline_row.scale,
                ),
                ValueCell::IconGlyph(rain_timeline_row.glyph()),
                rain_timeline_row.precipitation_type_cell(),
            ],
        )
    }
}

fn model_quarter_containing<'forecast>(
    quarters: &[&'forecast ModelQuarterHour],
    at: DateTime<Utc>,
) -> Option<&'forecast ModelQuarterHour> {
    quarters
        .iter()
        .copied()
        .find(|quarter| quarter.starts_at <= at && at < quarter.starts_at + MODEL_QUARTER_HOUR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimelineHour {
    date: NaiveDate,
    hour: u32,
}

pub(super) fn rain_timeline<'forecast>(
    rate_sources: Vec<RankedRateSource<'forecast>>,
    type_quarters: &[&'forecast ModelQuarterHour],
    scale: RainIntensityScale,
) -> Vec<RainTimelineRow<'forecast>> {
    rate_indicators_by_rank(rate_sources)
        .into_iter()
        .scan(None, |previous_quarter_start, sample| {
            let type_marker = match model_quarter_containing(type_quarters, sample.starts_at()) {
                Some(quarter) if *previous_quarter_start != Some(quarter.starts_at) => {
                    *previous_quarter_start = Some(quarter.starts_at);
                    PrecipitationTypeMarker::StartsQuarter(quarter)
                }
                _ => PrecipitationTypeMarker::Unmarked,
            };
            Some(RainTimelineRow {
                sample,
                type_marker,
                scale,
            })
        })
        .collect()
}

pub(super) fn rain_timeline_section(rows: &[RainTimelineRow<'_>]) -> Section {
    Section::new(
        "Timeline",
        headed_grouped_table(
            TIMELINE_HEADINGS,
            rows.chunk_by(|earlier, later| earlier.starting_hour() == later.starting_hour()),
        )
        .marked_as_continuing(),
    )
}
