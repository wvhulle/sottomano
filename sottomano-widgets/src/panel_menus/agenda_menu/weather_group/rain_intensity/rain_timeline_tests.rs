use sottomano_desktop_sources::weather::{
    open_meteo_precipitation_model::ModelQuarterHour,
    open_meteo_temperature::{Daylight, Intensity, WeatherCondition},
};

use super::{
    radar_rain_alert_tests::radar_frames,
    rain_strength_scale::RadarRainStrength,
    rain_timeline_rows::{PrecipitationTypeMarker, RainTimelineRow},
    *,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    panel_menus::agenda_menu::weather_group::{
        precipitation_type_forecast::MODEL_QUARTER_HOUR, rain_forecast_ranking::RainRateIndicator,
    },
    widgets::keyed_record_table::{RecordCells, TableRecord},
};

fn model_quarters(clock: Clock, conditions: &[WeatherCondition]) -> PrecipitationModel {
    let quarters: Vec<_> = (0..)
        .zip(conditions)
        .map(|(position, condition)| ModelQuarterHour {
            starts_at: clock.utc() + MODEL_QUARTER_HOUR * position,
            intensity: Velocity::new::<millimeter_per_hour>(match condition.sky_wetness() {
                SkyWetness::Precipitating => 1.2,
                SkyWetness::Dry => 0.0,
            }),
            condition: *condition,
            daylight: Daylight::Day,
        })
        .collect();
    PrecipitationModel {
        current: quarters[0].clone(),
        quarters,
    }
}

fn default_scale() -> RainIntensityScale {
    RainForecast {
        sources: RainSources::Radar(&radar_frames(Clock::real(), &[0.0])),
        alerts: &RainAlertsConfig::default(),
    }
    .intensity_scale()
}

fn type_cells(timeline: &[RainTimelineRow<'_>]) -> Vec<ValueCell> {
    timeline
        .iter()
        .map(|row| {
            RecordCells::from(TableRecord::<3>::from(row))
                .cells
                .get(2)
                .cloned()
                .unwrap()
        })
        .collect()
}

#[test]
fn radar_indicators_read_intensity_or_dry_without_a_model_type() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[2.0, 0.0]);
    let scale = default_scale();
    let row = |frame| {
        RecordCells::from(TableRecord::<3>::from(&RainTimelineRow {
            sample: RainRateIndicator::RadarFrame(frame),
            type_marker: PrecipitationTypeMarker::Unmarked,
            scale,
        }))
    };
    let wet = row(&rain.forecast[0]);
    assert_eq!(
        match wet.cells.first().unwrap() {
            ValueCell::Quantity(quantity) => quantity.localized_text(Locale::en_US),
            _ => String::new(),
        },
        "2.0 mm/h"
    );
    assert_eq!(
        wet.cells.get(1),
        Some(&ValueCell::IconGlyph(WeatherIcon::Rain.into()))
    );
    assert_eq!(wet.cells.get(2), Some(&ValueCell::Blank));
    let dry = row(&rain.forecast[1]);
    assert_eq!(dry.cells.first(), Some(&ValueCell::from("–")));
}

#[test]
fn the_model_names_the_type_where_both_see_rain_and_continues_past_the_radar() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0, 0.0, 0.4, 0.4, 0.4]);
    let drizzle = WeatherCondition::Drizzle(Intensity::Light);
    let model = model_quarters(clock, &[drizzle, drizzle, drizzle]);
    let alerts = RainAlertsConfig::default();
    let forecast = RainForecast {
        sources: RainSources::RadarAndModel {
            radar: &rain,
            model: &model,
        },
        alerts: &alerts,
    };
    let timeline = forecast.timeline(clock);
    assert_eq!(
        type_cells(&timeline),
        [
            ValueCell::Blank,
            ValueCell::Blank,
            ValueCell::Blank,
            ValueCell::from("Light drizzle"),
            ValueCell::Blank,
            ValueCell::Blank,
            ValueCell::from("Light drizzle"),
        ]
    );
    assert!(matches!(
        timeline.last().map(|row| row.sample),
        Some(RainRateIndicator::ModelQuarter(_))
    ));
}

#[test]
fn a_dry_model_stays_silent_beside_radar_rain() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.4, 0.4, 0.4]);
    let model = model_quarters(clock, &[WeatherCondition::Overcast]);
    let alerts = RainAlertsConfig::default();
    let forecast = RainForecast {
        sources: RainSources::RadarAndModel {
            radar: &rain,
            model: &model,
        },
        alerts: &alerts,
    };
    assert_eq!(
        type_cells(&forecast.timeline(clock)),
        [ValueCell::Blank, ValueCell::Blank, ValueCell::Blank]
    );
}

#[test]
fn a_wet_model_does_not_wet_a_dry_radar_frame() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0, 0.0]);
    let model = model_quarters(clock, &[WeatherCondition::Drizzle(Intensity::Light)]);
    let alerts = RainAlertsConfig::default();
    let forecast = RainForecast {
        sources: RainSources::RadarAndModel {
            radar: &rain,
            model: &model,
        },
        alerts: &alerts,
    };
    let timeline = forecast.timeline(clock);
    assert_eq!(
        type_cells(&timeline),
        [ValueCell::Blank, ValueCell::Blank, ValueCell::Blank]
    );
    assert!(
        timeline
            .iter()
            .all(|row| row.sample.sky_wetness() == SkyWetness::Dry)
    );
}

#[test]
fn without_radar_the_timeline_lists_model_quarters_only() {
    let clock = Clock::real();
    let model = model_quarters(
        clock,
        &[WeatherCondition::Overcast, WeatherCondition::Overcast],
    );
    let forecast = RainForecast::modelled_with_default_alerts(&model);
    let timeline = forecast.timeline(clock);
    assert_eq!(timeline.len(), 2);
    assert!(
        timeline
            .iter()
            .all(|row| matches!(row.sample, RainRateIndicator::ModelQuarter(_)))
    );
}

#[test]
fn without_a_model_the_timeline_lists_radar_frames_only() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 1.0, 0.0]);
    let forecast = RainForecast {
        sources: RainSources::Radar(&rain),
        alerts: &RainAlertsConfig::default(),
    };
    let timeline = forecast.timeline(clock);
    assert_eq!(timeline.len(), 3);
    assert!(
        timeline
            .iter()
            .all(|row| matches!(row.sample, RainRateIndicator::RadarFrame(_)))
    );
}

#[test]
fn source_rows_list_providers_from_most_to_least_trusted() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0]);
    let model = model_quarters(clock, &[WeatherCondition::Overcast]);
    let sources = RainSources::RadarAndModel {
        radar: &rain,
        model: &model,
    };
    assert_eq!(
        sources.source_rows(clock),
        vec![
            LabeledValue::new("Radar source", "Buienradar"),
            LabeledValue::new("Model source", "Open-Meteo"),
        ]
    );
}

#[test]
fn a_model_without_radar_names_the_type_only_while_wet() {
    let clock = Clock::real();
    let alerts = RainAlertsConfig::default();
    let snowing = model_quarters(clock, &[WeatherCondition::Snow(Intensity::Light)]);
    let snow_forecast = RainForecast {
        sources: RainSources::Model(&snowing),
        alerts: &alerts,
    };
    assert_eq!(
        snow_forecast.label_text(clock, Locale::default()),
        Some("Light snow".to_owned())
    );
    let clear = model_quarters(clock, &[WeatherCondition::MainlyClear]);
    let clear_forecast = RainForecast {
        sources: RainSources::Model(&clear),
        alerts: &alerts,
    };
    assert_eq!(clear_forecast.label_text(clock, Locale::default()), None);
    assert_eq!(
        clear_forecast.glyph(),
        IconGlyph::from(Phosphor::DropSlash(Weight::Regular))
    );
    assert_eq!(clear_forecast.display_state(clock), IndicatorState::Off);
}

#[test]
fn rain_strength_follows_the_met_office_and_the_configured_heavy_threshold() {
    let alerts = RainAlertsConfig {
        heavy_millimeters_per_hour: 10.0,
        ..RainAlertsConfig::default()
    };
    let rain = radar_frames(Clock::real(), &[0.0]);
    let scale = RadarRainForecast {
        rain: &rain,
        alerts: &alerts,
    }
    .intensity_scale();
    let strength = |millimeters| scale.strength(Velocity::new::<millimeter_per_hour>(millimeters));
    assert_eq!(strength(0.0), RadarRainStrength::Dry);
    assert_eq!(strength(0.4), RadarRainStrength::Light);
    assert_eq!(strength(0.5), RadarRainStrength::Moderate);
    assert_eq!(strength(9.9), RadarRainStrength::Moderate);
    assert_eq!(strength(10.0), RadarRainStrength::Heavy);
}

#[test]
fn intensity_prints_in_millimeters_per_hour() {
    assert_eq!(
        intensity_text(Velocity::new::<millimeter_per_hour>(2.5)),
        "2.5 mm/h"
    );
}
