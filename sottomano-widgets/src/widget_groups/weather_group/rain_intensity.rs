use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    iter,
    sync::LazyLock,
};

use chrono::{DateTime, Local, Locale, NaiveDate, Timelike, Utc};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::weather::{
    buienradar_rain::{Downpour, Rain, RainOutlook, precipitation::millimeter_per_hour},
    open_meteo_precipitation_model::{ModelQuarterHour, PrecipitationModel},
    open_meteo_temperature::SkyWetness,
};
use sottomano_toml_config::{severity::Severity, widget_groups::rain_alerts::RainAlertsConfig};
use uom::{fmt::DisplayStyle::Abbreviation, si::f64::Velocity};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::weather_group::{
        precipitation_type_forecast::{
            MODEL_QUARTER_HOUR, model_quarter_glyph, model_quarter_text, quarters_not_yet_over,
        },
        rain_forecast_ranking::{
            RainRateReading, RankedRainProvider, RankedRateSource, frames_not_yet_over,
            ranked_by_method, rate_readings_by_rank,
        },
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_durations::{duration_text, relative_moment_text},
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{definition_list, moment_text},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
        weather_icons::WeatherIcon,
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
const MET_OFFICE_MODERATE_RAIN_FROM_MILLIMETERS_PER_HOUR: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RadarRainStrength {
    Dry,
    Light,
    Moderate,
    Heavy,
}

impl RadarRainStrength {
    fn glyph(self) -> IconGlyph {
        match self {
            RadarRainStrength::Dry => Phosphor::DropSlash(Weight::Regular).into(),
            RadarRainStrength::Light => WeatherIcon::Sprinkle.into(),
            RadarRainStrength::Moderate => WeatherIcon::Rain.into(),
            RadarRainStrength::Heavy => WeatherIcon::Raindrops.into(),
        }
    }
}

impl Display for RadarRainStrength {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            RadarRainStrength::Dry => "Dry",
            RadarRainStrength::Light => "Light",
            RadarRainStrength::Moderate => "Moderate",
            RadarRainStrength::Heavy => "Heavy",
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct RainIntensityScale {
    heavy_from: Velocity,
}

impl RainIntensityScale {
    fn strength(self, intensity: Velocity) -> RadarRainStrength {
        if intensity <= Velocity::default() {
            RadarRainStrength::Dry
        } else if intensity
            < Velocity::new::<millimeter_per_hour>(
                MET_OFFICE_MODERATE_RAIN_FROM_MILLIMETERS_PER_HOUR,
            )
        {
            RadarRainStrength::Light
        } else if intensity < self.heavy_from {
            RadarRainStrength::Moderate
        } else {
            RadarRainStrength::Heavy
        }
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

const TIMELINE_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("Rain"),
    ColumnHeading::untitled(),
    ColumnHeading::titled("Type").align_x(Horizontal::Left),
];

#[derive(Debug, Clone, Copy)]
enum PrecipitationTypeMarker<'a> {
    Unmarked,
    StartsQuarter(&'a ModelQuarterHour),
}

#[derive(Debug, Clone, Copy)]
struct RainTimelineRow<'a> {
    reading: RainRateReading<'a>,
    type_marker: PrecipitationTypeMarker<'a>,
    scale: RainIntensityScale,
}

impl RainTimelineRow<'_> {
    fn starting_hour(self) -> TimelineHour {
        let starts_at = self.reading.starts_at().with_timezone(&Local);
        TimelineHour {
            date: starts_at.date_naive(),
            hour: starts_at.hour(),
        }
    }

    fn sky_wetness(self) -> SkyWetness {
        self.reading.sky_wetness()
    }

    fn glyph(self) -> IconGlyph {
        let strength_glyph = self.scale.strength(self.reading.intensity()).glyph();
        match self.reading {
            RainRateReading::RadarFrame(_) => strength_glyph,
            RainRateReading::ModelQuarter(quarter) => model_quarter_glyph(quarter, strength_glyph),
        }
    }

    fn precipitation_type_cell(self) -> ValueCell {
        match (self.reading, self.type_marker) {
            (RainRateReading::ModelQuarter(quarter), _) => model_quarter_text(quarter).into(),
            (reading, PrecipitationTypeMarker::StartsQuarter(quarter))
                if reading.sky_wetness() == SkyWetness::Precipitating
                    && quarter.condition.sky_wetness() == SkyWetness::Precipitating =>
            {
                model_quarter_text(quarter).into()
            }
            (RainRateReading::RadarFrame(_), _) => ValueCell::Blank,
        }
    }
}

fn rain_amount_cell(intensity: Velocity, scale: RainIntensityScale) -> ValueCell {
    match scale.strength(intensity) {
        RadarRainStrength::Dry => ValueCell::from("–"),
        _ => Quantity::precipitation_rate(intensity).into(),
    }
}

impl IntoTableRecord<3> for &RainTimelineRow<'_> {
    fn into_table_record(self) -> TableRecord<3> {
        let time = ValueCell::Time {
            at: self.reading.starts_at(),
            format: LocalizedTimeFormat::HourAndMinute,
        };
        TableRecord::new(
            time,
            [
                rain_amount_cell(self.reading.intensity(), self.scale),
                ValueCell::IconGlyph(self.glyph()),
                self.precipitation_type_cell(),
            ],
        )
    }
}

fn model_quarter_containing<'a>(
    quarters: &[&'a ModelQuarterHour],
    at: DateTime<Utc>,
) -> Option<&'a ModelQuarterHour> {
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

fn rain_timeline<'a>(
    rate_sources: Vec<RankedRateSource<'a>>,
    type_quarters: &[&'a ModelQuarterHour],
    scale: RainIntensityScale,
) -> Vec<RainTimelineRow<'a>> {
    rate_readings_by_rank(rate_sources)
        .into_iter()
        .scan(None, |previous_quarter_start, reading| {
            let type_marker = match model_quarter_containing(type_quarters, reading.starts_at()) {
                Some(quarter) if *previous_quarter_start != Some(quarter.starts_at) => {
                    *previous_quarter_start = Some(quarter.starts_at);
                    PrecipitationTypeMarker::StartsQuarter(quarter)
                }
                _ => PrecipitationTypeMarker::Unmarked,
            };
            Some(RainTimelineRow {
                reading,
                type_marker,
                scale,
            })
        })
        .collect()
}

fn rain_timeline_section(rows: &[RainTimelineRow<'_>]) -> Section {
    Section::grouped_column_table(
        TIMELINE_HEADINGS,
        rows.chunk_by(|earlier, later| earlier.starting_hour() == later.starting_hour()),
    )
    .marked_as_continuing()
}

#[derive(Debug, Clone, Copy)]
pub enum RainSources<'a> {
    RadarAndModel {
        radar: &'a Rain,
        model: &'a PrecipitationModel,
    },
    Radar(&'a Rain),
    Model(&'a PrecipitationModel),
}

impl<'a> RainSources<'a> {
    #[must_use]
    fn radar(self) -> Option<&'a Rain> {
        match self {
            RainSources::RadarAndModel { radar, .. } | RainSources::Radar(radar) => Some(radar),
            RainSources::Model(_) => None,
        }
    }

    fn model(self) -> Option<&'a PrecipitationModel> {
        match self {
            RainSources::RadarAndModel { model, .. } | RainSources::Model(model) => Some(model),
            RainSources::Radar(_) => None,
        }
    }

    fn rate_sources(self, clock: Clock) -> Vec<RankedRateSource<'a>> {
        let radar_source = self.radar().map(|rain| rain.ranked_rate_source(clock));
        let model_source = self.model().map(|model| model.ranked_rate_source(clock));
        radar_source.into_iter().chain(model_source).collect()
    }

    fn caption(self, clock: Clock) -> String {
        ranked_by_method(self.rate_sources(clock))
            .iter()
            .map(|source| format!("{}: {}", source.method, source.provider_name))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

pub struct RainForecast<'a> {
    pub sources: RainSources<'a>,
    pub alerts: &'a RainAlertsConfig,
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
        let entries: Vec<LabeledValue> = match (self.sources.radar(), self.sources.model()) {
            (Some(rain), _) => match (rain.outlook, rain.forecast.last()) {
                (RainOutlook::Dry, Some(last_frame)) => vec![
                    LabeledValue::new("Rain", "none expected"),
                    LabeledValue::new("Dry until at least", time_of_day(last_frame.at)),
                ],
                (outlook, _) => {
                    let peak = &rain.peak;
                    iter::once(LabeledValue::new(
                        "Now",
                        current_intensity_cell(rain.intensity),
                    ))
                    .chain(
                        outlook_turn(outlook)
                            .map(|turn| LabeledValue::new(turn.label, time_of_day(turn.at))),
                    )
                    .chain([
                        LabeledValue::new("Peak", Quantity::precipitation_rate(peak.intensity)),
                        LabeledValue::new("Peak at", time_of_day(peak.at)),
                    ])
                    .collect()
                }
            },
            (None, Some(model))
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
            (None, Some(_)) => vec![LabeledValue::new("Now", "Dry")],
            (None, None) => Vec::new(),
        };
        let title = match self.sources {
            RainSources::Model(_) => "Next 3 hours",
            RainSources::RadarAndModel { .. } | RainSources::Radar(_) => "Next 2 hours",
        };
        Section::dictionary(entries).titled((title, self.sources.caption(clock)))
    }
}

impl GroupItemReading for RainForecast<'_> {
    fn name(&self) -> &'static str {
        "Rain"
    }

    fn icon(&self) -> IconGlyph {
        match (self.radar_forecast(), self.sources.model()) {
            (Some(radar), _) => radar.icon(),
            (None, Some(model)) => model_quarter_glyph(
                &model.current,
                self.intensity_scale()
                    .strength(model.current.intensity)
                    .glyph(),
            ),
            (None, None) => Self::fallback_icon(),
        }
    }

    fn fallback_icon() -> IconGlyph {
        Rain::fallback_icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        match (self.radar_forecast(), self.sources.model()) {
            (Some(radar), _) => radar.label(clock, locale),
            (None, Some(model)) => (model.current.condition.sky_wetness()
                == SkyWetness::Precipitating)
                .then(|| model.current.condition.to_string()),
            (None, None) => None,
        }
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        match (self.radar_forecast(), self.sources.model()) {
            (Some(radar), _) => radar.state(clock),
            (None, Some(model))
                if model.current.condition.sky_wetness() == SkyWetness::Precipitating =>
            {
                ReadingDisplayState::Idle
            }
            (None, _) => ReadingDisplayState::Off,
        }
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        match self.radar_forecast() {
            Some(radar) => radar.ongoing_toast_threshold(),
            None => OngoingToastThreshold::NeverNotify,
        }
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.radar_forecast()?.ongoing_toast_body(clock, locale)
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let timeline = self.timeline(clock);
        let timeline_section = timeline
            .iter()
            .any(|row| row.sky_wetness() == SkyWetness::Precipitating)
            .then(|| rain_timeline_section(&timeline));
        HoverCardBody::sections(iter::once(self.summary_section(clock)).chain(timeline_section))
    }
}

pub struct RadarRainForecast<'a> {
    pub rain: &'a Rain,
    pub alerts: &'a RainAlertsConfig,
}

static DEFAULT_RAIN_ALERTS: LazyLock<RainAlertsConfig> = LazyLock::new(RainAlertsConfig::default);

impl<'a> RadarRainForecast<'a> {
    fn with_default_alerts(rain: &'a Rain) -> Self {
        RadarRainForecast {
            rain,
            alerts: &DEFAULT_RAIN_ALERTS,
        }
    }
}

impl<'a> RainForecast<'a> {
    fn modelled_with_default_alerts(model: &'a PrecipitationModel) -> Self {
        RainForecast {
            sources: RainSources::Model(model),
            alerts: &DEFAULT_RAIN_ALERTS,
        }
    }
}

struct RainSpell<'a> {
    outlook: RainOutlook,
    starts_at: DateTime<Local>,
    peak: &'a Downpour,
}

impl RadarRainForecast<'_> {
    fn remaining_frames(&self, now: DateTime<Local>) -> &[Downpour] {
        frames_not_yet_over(&self.rain.forecast, now)
    }

    fn intensity_scale(&self) -> RainIntensityScale {
        RainIntensityScale {
            heavy_from: Velocity::new::<millimeter_per_hour>(
                self.alerts.heavy_millimeters_per_hour,
            ),
        }
    }

    fn spell_above_minimum(&self, now: DateTime<Local>) -> Option<RainSpell<'_>> {
        let minimum =
            Velocity::new::<millimeter_per_hour>(self.alerts.minimum_millimeters_per_hour);
        let remaining = self.remaining_frames(now);
        let outlook = RainOutlook::of_rain_above(remaining, minimum);
        let starts_at = match outlook {
            RainOutlook::Dry => return None,
            RainOutlook::StartsAt(at) => at,
            RainOutlook::StopsAt(_) | RainOutlook::OutlastsForecastUntil(_) => {
                remaining.first()?.at
            }
        };
        let peak = remaining
            .iter()
            .skip_while(|frame| frame.at < starts_at)
            .take_while(|frame| frame.intensity > minimum)
            .max_by(|earlier, later| earlier.intensity.value.total_cmp(&later.intensity.value))?;
        Some(RainSpell {
            outlook,
            starts_at,
            peak,
        })
    }

    fn alerting_spell(&self, now: DateTime<Local>) -> Option<RainSpell<'_>> {
        let lead_time = self.alerts.lead_time;
        self.spell_above_minimum(now)
            .filter(|spell| spell.starts_at - now <= lead_time)
    }

    fn severity(&self, now: DateTime<Local>) -> Severity {
        let heavy = Velocity::new::<millimeter_per_hour>(self.alerts.heavy_millimeters_per_hour);
        match self.alerting_spell(now) {
            Some(spell) if spell.peak.intensity >= heavy => Severity::Critical,
            Some(_) => Severity::Warning,
            None => Severity::Normal,
        }
    }
}

impl GroupItemReading for RadarRainForecast<'_> {
    fn name(&self) -> &'static str {
        self.rain.name()
    }

    fn icon(&self) -> IconGlyph {
        self.intensity_scale()
            .strength(self.rain.peak.intensity)
            .glyph()
    }

    fn fallback_icon() -> IconGlyph {
        Rain::fallback_icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.rain.label(clock, locale)
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        match self.severity(clock.local()) {
            Severity::Normal => self.rain.state(clock),
            alerting => alerting.into(),
        }
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Rain on the radar",
        }
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        let now = clock.local();
        let spell = self.spell_above_minimum(now)?;
        let moment = |at: DateTime<Local>| moment_text(at.to_utc(), clock.utc(), locale);
        let turn = outlook_turn(spell.outlook).map(|turn| (turn.label, moment(turn.at)));
        let current_intensity = self
            .remaining_frames(now)
            .first()
            .map_or_else(Velocity::default, |frame| frame.intensity);
        Some(definition_list(turn.into_iter().chain([
            ("Now", current_intensity_text(current_intensity)),
            ("Peak", intensity_text(spell.peak.intensity)),
            ("Peak at", moment(spell.peak.at)),
        ])))
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        RainForecast {
            sources: RainSources::Radar(self.rain),
            alerts: self.alerts,
        }
        .hover_card(clock)
    }
}

impl GroupItemReading for Rain {
    fn name(&self) -> &'static str {
        "Rain"
    }

    fn icon(&self) -> IconGlyph {
        RadarRainForecast::with_default_alerts(self).icon()
    }

    fn fallback_icon() -> IconGlyph {
        WeatherIcon::Raindrop.into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        let remaining = |at: DateTime<Local>| duration_text(at - clock.local(), locale);
        match self.outlook {
            RainOutlook::Dry => None,
            RainOutlook::StartsAt(at) => {
                Some(relative_moment_text(at.to_utc(), clock.utc(), locale))
            }
            RainOutlook::StopsAt(at) => Some(format!("for {}", remaining(at))),
            RainOutlook::OutlastsForecastUntil(at) => Some(format!("for {}+", remaining(at))),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.outlook == RainOutlook::Dry {
            ReadingDisplayState::Off
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        RadarRainForecast::with_default_alerts(self).hover_card(clock)
    }
}

impl GroupItemReading for PrecipitationModel {
    fn name(&self) -> &'static str {
        "Precipitation model"
    }

    fn icon(&self) -> IconGlyph {
        RainForecast::modelled_with_default_alerts(self).icon()
    }

    fn fallback_icon() -> IconGlyph {
        WeatherIcon::Sprinkle.into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        RainForecast::modelled_with_default_alerts(self).label(clock, locale)
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        RainForecast::modelled_with_default_alerts(self).hover_card(clock)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::weather::open_meteo_temperature::{
        Daylight, Intensity, WeatherCondition,
    };
    use uom::si::{
        f64::{Length, Time},
        length::millimeter,
        time::hour,
    };

    use super::*;
    use crate::{
        group_items::group_item_reading::PanelVisibility,
        hover_cards::HoverTarget,
        widget_groups::weather_group::{WeatherTarget, rain_forecast_ranking::RADAR_FRAME_LENGTH},
        widgets::{keyed_record_table::RecordCells, ongoing_toast::OngoingToast},
    };

    #[test]
    fn millimeter_per_hour_is_a_millimeter_each_hour() {
        let from_unit = Velocity::new::<millimeter_per_hour>(2.5);
        let from_parts = Length::new::<millimeter>(2.5) / Time::new::<hour>(1.0);
        assert!((from_unit - from_parts).abs() < Velocity::new::<millimeter_per_hour>(1e-9));
    }

    fn radar_frames(clock: Clock, millimeters_per_hour: &[f64]) -> Rain {
        Rain::from_forecast(
            (0..)
                .zip(millimeters_per_hour)
                .map(|(frame, intensity)| Downpour {
                    intensity: Velocity::new::<millimeter_per_hour>(*intensity),
                    at: clock.local() + RADAR_FRAME_LENGTH * frame,
                })
                .collect(),
        )
        .unwrap()
    }

    fn severity_of(rain: &Rain, clock: Clock) -> Severity {
        RadarRainForecast::with_default_alerts(rain).severity(clock.local())
    }

    #[test]
    fn a_dry_forecast_never_alerts() {
        let clock = Clock::real();
        assert_eq!(
            severity_of(&radar_frames(clock, &[0.0, 0.0]), clock),
            Severity::Normal
        );
    }

    #[test]
    fn a_dry_forecast_stays_on_the_panel_as_inactive() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0]);
        let forecast = RadarRainForecast::with_default_alerts(&rain);
        assert_eq!(forecast.panel_visibility(), PanelVisibility::Shown);
        assert_eq!(forecast.state(clock), ReadingDisplayState::Off);
        assert_eq!(
            forecast.icon(),
            IconGlyph::from(Phosphor::DropSlash(Weight::Regular))
        );
        assert_eq!(
            RadarRainForecast::with_default_alerts(&radar_frames(clock, &[0.0, 0.2])).state(clock),
            ReadingDisplayState::Idle
        );
    }

    #[test]
    fn rain_beyond_the_lead_time_waits_until_it_comes_closer() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0]);
        assert_eq!(severity_of(&rain, clock), Severity::Normal);
        assert_eq!(
            severity_of(&rain, clock.advanced(Duration::from_mins(30))),
            Severity::Warning
        );
    }

    #[test]
    fn heavy_rain_within_the_lead_time_is_critical() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0, 12.0]);
        assert_eq!(severity_of(&rain, clock), Severity::Critical);
    }

    #[test]
    fn drizzle_below_the_minimum_stays_quiet() {
        let clock = Clock::real();
        assert_eq!(
            severity_of(&radar_frames(clock, &[0.2, 0.3, 0.2, 0.0]), clock),
            Severity::Normal
        );
        assert_eq!(
            severity_of(&radar_frames(clock, &[2.0, 2.0, 0.0]), clock),
            Severity::Warning
        );
    }

    #[test]
    fn drizzle_now_does_not_bring_a_far_shower_forward() {
        let clock = Clock::real();
        let mut frames = vec![0.2; 20];
        frames.push(12.0);
        assert_eq!(
            severity_of(&radar_frames(clock, &frames), clock),
            Severity::Normal
        );
    }

    #[test]
    fn the_toast_describes_the_shower_rather_than_the_drizzle_before_it() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.2, 0.2, 3.0, 1.0, 0.0, 9.0]);
        let body = RadarRainForecast::with_default_alerts(&rain)
            .ongoing_toast_body(clock, Locale::default())
            .unwrap();
        assert_eq!(severity_of(&rain, clock), Severity::Warning);
        assert!(body.contains("<dt>Starts</dt>"));
        assert!(body.contains("<dt>Peak</dt><dd>3.0 mm/h</dd>"));
    }

    #[test]
    fn a_forecast_kept_through_failed_fetches_ages_out() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[3.0, 3.0, 3.0]);
        assert_eq!(severity_of(&rain, clock), Severity::Warning);
        assert_eq!(
            severity_of(&rain, clock.advanced(Duration::from_mins(15))),
            Severity::Normal
        );
    }

    #[test]
    fn the_label_counts_down_with_the_clock() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0, 3.0]);
        let later = clock.advanced(Duration::from_mins(4));
        assert_eq!(
            rain.label(later, Locale::default()),
            Some("in 6 min.".to_owned())
        );
    }

    #[test]
    fn the_toast_body_stays_the_same_as_the_clock_advances() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0, 3.0]);
        let forecast = RadarRainForecast::with_default_alerts(&rain);
        let body = forecast.ongoing_toast_body(clock, Locale::default());
        assert!(
            body.as_deref()
                .is_some_and(|body| body.contains("<dt>Starts</dt>"))
        );
        assert_eq!(
            forecast.ongoing_toast_body(clock.advanced(Duration::from_secs(50)), Locale::default()),
            body
        );
    }

    #[test]
    fn rain_starting_soon_raises_a_toast() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 3.0]);
        let toast = OngoingToast::from_reading_display_state(
            HoverTarget::Weather(WeatherTarget::Rain),
            &RadarRainForecast::with_default_alerts(&rain),
            0_u8,
            Locale::default(),
            clock,
        );
        assert_eq!(toast.map(|toast| toast.severity), Some(Severity::Warning));
    }

    #[test]
    fn the_schedule_drops_frames_already_over() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[1.0, 2.0, 3.0, 0.0]);
        let schedule = frames_not_yet_over(
            &rain.forecast,
            clock.advanced(Duration::from_mins(7)).local(),
        );
        assert_eq!(schedule.len(), 3);
        assert_eq!(schedule[0], rain.forecast[1]);
    }

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

    fn timeline_of<'a>(forecast: &'a RainForecast<'a>, clock: Clock) -> Vec<RainTimelineRow<'a>> {
        forecast.timeline(clock)
    }

    fn type_cells(timeline: &[RainTimelineRow<'_>]) -> Vec<ValueCell> {
        timeline
            .iter()
            .map(|row| {
                RecordCells::from(row.into_table_record())
                    .cell(2)
                    .cloned()
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn radar_rows_read_intensity_or_dry_without_a_model_type() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[2.0, 0.0]);
        let scale = default_scale();
        let row = |frame| {
            RecordCells::from(
                (&RainTimelineRow {
                    reading: RainRateReading::RadarFrame(frame),
                    type_marker: PrecipitationTypeMarker::Unmarked,
                    scale,
                })
                    .into_table_record(),
            )
        };
        let wet = row(&rain.forecast[0]);
        assert_eq!(
            match wet.cell(0).unwrap() {
                ValueCell::Quantity(quantity) => quantity.localized_text(Locale::en_US),
                _ => String::new(),
            },
            "2.0 mm/h"
        );
        assert_eq!(
            wet.cell(1),
            Some(&ValueCell::IconGlyph(WeatherIcon::Rain.into()))
        );
        assert_eq!(wet.cell(2), Some(&ValueCell::Blank));
        let dry = row(&rain.forecast[1]);
        assert_eq!(dry.cell(0), Some(&ValueCell::from("–")));
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
        let timeline = timeline_of(&forecast, clock);
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
            timeline.last().map(|row| row.reading),
            Some(RainRateReading::ModelQuarter(_))
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
            type_cells(&timeline_of(&forecast, clock)),
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
        let timeline = timeline_of(&forecast, clock);
        assert_eq!(
            type_cells(&timeline),
            [ValueCell::Blank, ValueCell::Blank, ValueCell::Blank]
        );
        assert!(
            timeline
                .iter()
                .all(|row| row.sky_wetness() == SkyWetness::Dry)
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
        let timeline = timeline_of(&forecast, clock);
        assert_eq!(timeline.len(), 2);
        assert!(
            timeline
                .iter()
                .all(|row| matches!(row.reading, RainRateReading::ModelQuarter(_)))
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
        let timeline = timeline_of(&forecast, clock);
        assert_eq!(timeline.len(), 3);
        assert!(
            timeline
                .iter()
                .all(|row| matches!(row.reading, RainRateReading::RadarFrame(_)))
        );
    }

    #[test]
    fn the_caption_lists_providers_from_most_to_least_trusted() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0]);
        let model = model_quarters(clock, &[WeatherCondition::Overcast]);
        let sources = RainSources::RadarAndModel {
            radar: &rain,
            model: &model,
        };
        assert_eq!(
            sources.caption(clock),
            "radar: Buienradar · model: Open-Meteo"
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
            snow_forecast.label(clock, Locale::default()),
            Some("Light snow".to_owned())
        );
        let clear = model_quarters(clock, &[WeatherCondition::MainlyClear]);
        let clear_forecast = RainForecast {
            sources: RainSources::Model(&clear),
            alerts: &alerts,
        };
        assert_eq!(clear_forecast.label(clock, Locale::default()), None);
        assert_eq!(
            clear_forecast.icon(),
            IconGlyph::from(Phosphor::DropSlash(Weight::Regular))
        );
        assert_eq!(clear_forecast.state(clock), ReadingDisplayState::Off);
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
        let strength =
            |millimeters| scale.strength(Velocity::new::<millimeter_per_hour>(millimeters));
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
}
