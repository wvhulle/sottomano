mod precipitation_type_forecast;
mod rain_forecast_ranking;
mod rain_row;
use std::iter;

use chrono::{DateTime, Local, Locale};
use sottomano_desktop_sources::weather::{
    buienradar_rain::{Rain, RainOutlook, precipitation::millimeter_per_hour},
    open_meteo_precipitation_model::PrecipitationModel,
    open_meteo_temperature::SkyWetness,
};
use sottomano_toml_config::{
    panel_menus::agenda_menu::rain_alerts::{DEFAULT_RAIN_ALERTS, RainAlertsConfig},
    severity::Severity,
};
use uom::{fmt::DisplayStyle::Abbreviation, si::f64::Velocity};

#[cfg(test)]
mod radar_rain_alert_tests;
mod radar_rain_forecast;
mod rain_strength_scale;
mod rain_timeline_rows;
#[cfg(test)]
mod rain_timeline_tests;

use self::{
    radar_rain_forecast::{RadarRainForecast, rain_label_text},
    rain_strength_scale::RainIntensityScale,
    rain_timeline_rows::{RainTimelineRow, rain_timeline, rain_timeline_section},
};
use crate::{
    clock::Clock,
    glyph_icons::{icon_glyph::IconGlyph, weather_icons::WeatherIcon},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::agenda_menu::weather_group::rain_indicator::{
        precipitation_type_forecast::{
            model_quarter_glyph, model_quarter_text, quarters_not_yet_over,
        },
        rain_forecast_ranking::{
            RainForecastMethod, RankedRainProvider, RankedRateSource, ranked_by_method,
        },
    },
    widgets::{
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{IndicatorAlerts, NotificationBody, NotificationProgress, StateToast},
        value_cells::{LabeledValue, ValueCell},
    },
};

fn intensity_text(intensity: Velocity) -> String {
    format!(
        "{:.1}",
        intensity.into_format_args(millimeter_per_hour, Abbreviation)
    )
}

fn current_intensity_text(intensity: Velocity) -> String {
    if intensity <= Velocity::default() {
        "none right now".to_string()
    } else {
        intensity_text(intensity)
    }
}

fn current_intensity_cell(intensity: Velocity) -> ValueCell {
    if intensity <= Velocity::default() {
        "none right now".into()
    } else {
        Quantity::precipitation_rate(intensity).into()
    }
}

struct OutlookTurn {
    label: &'static str,
    at: DateTime<Local>,
}

fn outlook_turn(outlook: RainOutlook) -> Option<OutlookTurn> {
    let (label, at) = match outlook {
        RainOutlook::Dry => return None,
        RainOutlook::StartsAt(at) => ("Starts", at),
        RainOutlook::StopsAt(at) => ("Stops", at),
        RainOutlook::OutlastsForecastUntil(at) => ("Lasts past", at),
    };
    Some(OutlookTurn { label, at })
}

#[derive(Debug, Clone, Copy)]
pub enum RainSources<'forecast> {
    RadarAndModel {
        radar: &'forecast Rain,
        model: &'forecast PrecipitationModel,
    },
    Radar(&'forecast Rain),
    Model(&'forecast PrecipitationModel),
}

impl<'forecast> RainSources<'forecast> {
    #[must_use]
    fn radar(self) -> Option<&'forecast Rain> {
        match self {
            RainSources::RadarAndModel { radar, .. } | RainSources::Radar(radar) => Some(radar),
            RainSources::Model(_) => None,
        }
    }

    fn model(self) -> Option<&'forecast PrecipitationModel> {
        match self {
            RainSources::RadarAndModel { model, .. } | RainSources::Model(model) => Some(model),
            RainSources::Radar(_) => None,
        }
    }

    fn rate_sources(self, clock: Clock) -> Vec<RankedRateSource<'forecast>> {
        let radar_source = self.radar().map(|rain| rain.ranked_rate_source(clock));
        let model_source = self.model().map(|model| model.ranked_rate_source(clock));
        radar_source.into_iter().chain(model_source).collect()
    }

    fn source_rows(self, clock: Clock) -> Vec<LabeledValue> {
        ranked_by_method(self.rate_sources(clock))
            .iter()
            .map(|source| LabeledValue::new(method_source_key(source.method), source.provider_name))
            .collect()
    }
}

pub struct RainForecast<'forecast> {
    pub sources: RainSources<'forecast>,
    pub alerts: &'forecast RainAlertsConfig,
}

impl RainForecast<'_> {
    fn radar_forecast(&self) -> Option<RadarRainForecast<'_>> {
        Some(RadarRainForecast {
            rain: self.sources.radar()?,
            alerts: self.alerts,
        })
    }

    fn intensity_scale(&self) -> RainIntensityScale {
        RainIntensityScale {
            heavy_from: Velocity::new::<millimeter_per_hour>(
                self.alerts.heavy_millimeters_per_hour,
            ),
        }
    }

    fn timeline(&self, clock: Clock) -> Vec<RainTimelineRow<'_>> {
        let type_quarters: Vec<_> = self
            .sources
            .model()
            .into_iter()
            .flat_map(|model| quarters_not_yet_over(model, clock.utc()))
            .collect();
        rain_timeline(
            self.sources.rate_sources(clock),
            &type_quarters,
            self.intensity_scale(),
        )
    }

    fn summary_section(&self, clock: Clock) -> Section {
        let time_of_day = |at: DateTime<Local>| ValueCell::Time {
            at: at.to_utc(),
            format: LocalizedTimeFormat::HourAndMinute,
        };
        let entries: Vec<LabeledValue> = match self.sources.radar() {
            Some(rain) => match rain.forecast.last() {
                Some(last_frame) if rain.outlook == RainOutlook::Dry => vec![
                    LabeledValue::new("Rain", "none expected"),
                    LabeledValue::new("Dry until at least", time_of_day(last_frame.at)),
                ],
                _ => {
                    let peak = &rain.peak;
                    iter::once(LabeledValue::new(
                        "Now",
                        current_intensity_cell(rain.intensity),
                    ))
                    .chain(
                        outlook_turn(rain.outlook)
                            .map(|turn| LabeledValue::new(turn.label, time_of_day(turn.at))),
                    )
                    .chain([
                        LabeledValue::new("Peak", Quantity::precipitation_rate(peak.intensity)),
                        LabeledValue::new("Peak at", time_of_day(peak.at)),
                    ])
                    .collect()
                }
            },
            None => match self.sources.model() {
                Some(model)
                    if model.current.condition.sky_wetness() == SkyWetness::Precipitating =>
                {
                    vec![
                        LabeledValue::new("Now", model_quarter_text(&model.current)),
                        LabeledValue::new(
                            "Rain",
                            Quantity::precipitation_rate(model.current.intensity),
                        ),
                    ]
                }
                Some(_) => vec![LabeledValue::new("Now", "Dry")],
                None => Vec::new(),
            },
        };
        let title = match self.sources {
            RainSources::Model(_) => "Next 3 hours",
            RainSources::RadarAndModel { .. } | RainSources::Radar(_) => "Next 2 hours",
        };
        Section::new(
            title,
            dictionary_table(entries.into_iter().chain(self.sources.source_rows(clock))),
        )
    }
}

fn method_source_key(method: RainForecastMethod) -> &'static str {
    match method {
        RainForecastMethod::RadarNowcast => "Radar source",
        RainForecastMethod::NumericalWeatherModel => "Model source",
    }
}

impl<'forecast> RainForecast<'forecast> {
    pub fn indicator(self, clock: Clock, locale: Locale) -> GroupIndicator<'forecast> {
        let label = match self.label_text(clock, locale) {
            Some(label) if !label.is_empty() => ReadoutLabel::Labeled(label),
            Some(_) | None => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only("Rain", self.glyph(), self.display_state(clock))
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_alerts(IndicatorAlerts {
                state_toast: self.state_toast(clock, locale),
                ..IndicatorAlerts::default()
            })
            .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    fn glyph(&self) -> IconGlyph {
        match self.radar_forecast() {
            Some(radar) => radar.glyph(),
            None => match self.sources.model() {
                Some(model) => model_quarter_glyph(
                    &model.current,
                    self.intensity_scale()
                        .strength(model.current.intensity)
                        .glyph(),
                ),
                None => RAIN_FALLBACK_GLYPH,
            },
        }
    }

    fn label_text(&self, clock: Clock, locale: Locale) -> Option<String> {
        if let Some(radar) = self.radar_forecast() {
            return rain_label_text(radar.rain, clock, locale);
        }
        let model = self.sources.model()?;
        (model.current.condition.sky_wetness() == SkyWetness::Precipitating)
            .then(|| model.current.condition.to_string())
    }

    fn display_state(&self, clock: Clock) -> IndicatorState {
        match self.radar_forecast() {
            Some(radar) => radar.display_state(clock),
            None => match self.sources.model() {
                Some(model)
                    if model.current.condition.sky_wetness() == SkyWetness::Precipitating =>
                {
                    IndicatorState::Idle
                }
                _ => IndicatorState::Off,
            },
        }
    }

    fn state_toast(&self, clock: Clock, locale: Locale) -> StateToast {
        match self.radar_forecast() {
            Some(radar) => StateToast::NotifyFromSeverity {
                minimum_severity: Severity::Warning,
                summary_text: "Rain on the radar",
                body: radar
                    .toast_body(clock, locale)
                    .map_or(NotificationBody::Bodiless, NotificationBody::Text),
                progress: NotificationProgress::Unmeasured,
            },
            None => StateToast::NeverNotify,
        }
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let timeline = self.timeline(clock);
        let timeline_section = timeline
            .iter()
            .any(|row| row.sample.sky_wetness() == SkyWetness::Precipitating)
            .then(|| rain_timeline_section(&timeline));
        iter::once(self.summary_section(clock))
            .chain(timeline_section)
            .collect()
    }
}

impl<'forecast> RainForecast<'forecast> {
    fn modelled_with_default_alerts(model: &'forecast PrecipitationModel) -> Self {
        RainForecast {
            sources: RainSources::Model(model),
            alerts: &DEFAULT_RAIN_ALERTS,
        }
    }
}

pub(crate) const RAIN_FALLBACK_GLYPH: IconGlyph = IconGlyph::WeatherIcon(WeatherIcon::Raindrop);

pub fn rain_hover_card(rain: &Rain, clock: Clock) -> Vec<Section> {
    RadarRainForecast::with_default_alerts(rain).hover_card(clock)
}

pub(crate) const PRECIPITATION_MODEL_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::WeatherIcon(WeatherIcon::Sprinkle);

pub fn precipitation_model_hover_card(model: &PrecipitationModel, clock: Clock) -> Vec<Section> {
    RainForecast::modelled_with_default_alerts(model).hover_card(clock)
}
