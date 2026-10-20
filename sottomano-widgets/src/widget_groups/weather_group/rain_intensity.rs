use chrono::{DateTime, Local, Locale, TimeDelta};
use sottomano_config::{severity::Severity, widget_groups::rain_alerts::RainAlertsConfig};
use sottomano_sources::weather::buienradar_rain::{
    Downpour, Rain, RainOutlook, precipitation::millimeter_per_hour,
};
use uom::{fmt::DisplayStyle::Abbreviation, si::f64::Velocity};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{definition_list, moment_text},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, TooltipBody},
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
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

fn minutes_until(at: DateTime<Local>, clock: Clock) -> i64 {
    (at - clock.local()).num_minutes().max(0)
}

const RADAR_FRAME_LENGTH: TimeDelta = TimeDelta::minutes(5);

pub struct RainForecast<'a> {
    pub rain: &'a Rain,
    pub alerts: &'a RainAlertsConfig,
}

struct RainSpell<'a> {
    outlook: RainOutlook,
    starts_at: DateTime<Local>,
    peak: &'a Downpour,
}

impl RainForecast<'_> {
    fn remaining_frames(&self, now: DateTime<Local>) -> &[Downpour] {
        let elapsed = self
            .rain
            .forecast
            .iter()
            .take_while(|frame| frame.at + RADAR_FRAME_LENGTH <= now)
            .count();
        &self.rain.forecast[elapsed..]
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
            .take_while(|frame| frame.exceeds(minimum))
            .max_by(|earlier, later| earlier.intensity.value.total_cmp(&later.intensity.value))?;
        Some(RainSpell {
            outlook,
            starts_at,
            peak,
        })
    }

    fn alerting_spell(&self, now: DateTime<Local>) -> Option<RainSpell<'_>> {
        let lead_time = TimeDelta::from_std(self.alerts.lead_time).ok()?;
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

impl BarWidget for RainForecast<'_> {
    fn name(&self) -> &'static str {
        self.rain.name()
    }

    fn icon(&self) -> Glyph {
        self.rain.icon()
    }

    fn fallback_icon() -> Glyph {
        Rain::fallback_icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.rain.label(clock, locale)
    }

    fn state(&self, clock: Clock) -> WidgetState {
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
        let turn = match spell.outlook {
            RainOutlook::Dry => None,
            RainOutlook::StartsAt(at) => Some(("Starts", moment(at))),
            RainOutlook::StopsAt(at) => Some(("Stops", moment(at))),
            RainOutlook::OutlastsForecastUntil(at) => Some(("Lasts past", moment(at))),
        };
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

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        self.rain.tooltip(clock)
    }
}

impl BarWidget for Rain {
    fn name(&self) -> &'static str {
        "Rain"
    }

    fn icon(&self) -> Glyph {
        let peak = self.peak.intensity;
        if self.outlook == RainOutlook::Dry {
            Phosphor::DropSlash(Weight::Regular).into()
        } else if peak < Velocity::new::<millimeter_per_hour>(0.5) {
            WeatherIcon::Raindrop.into()
        } else if peak < Velocity::new::<millimeter_per_hour>(7.5) {
            WeatherIcon::Rain.into()
        } else {
            WeatherIcon::Thunderstorm.into()
        }
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::Raindrop.into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        match self.outlook {
            RainOutlook::Dry => None,
            RainOutlook::StartsAt(at) => Some(format!("in {}m", minutes_until(at, clock))),
            RainOutlook::StopsAt(at) => Some(format!("for {}m", minutes_until(at, clock))),
            RainOutlook::OutlastsForecastUntil(at) => {
                Some(format!("for {}m+", minutes_until(at, clock)))
            }
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.outlook == RainOutlook::Dry {
            WidgetState::Off
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let time_of_day = |at: DateTime<Local>| Cell::Time {
            at: at.to_utc(),
            format: LocalizedTimeFormat::TimeOfDay,
        };
        if let (RainOutlook::Dry, Some(last_frame)) = (self.outlook, self.forecast.last()) {
            return TooltipBody::dictionary([
                KeyValue::new("Rain", "none expected"),
                KeyValue::new("Dry until at least", time_of_day(last_frame.at)),
            ]);
        }
        TooltipBody::dictionary([
            KeyValue::new("Rain", current_intensity_text(self.intensity)),
            KeyValue::new("Peak", intensity_text(self.peak.intensity)),
            KeyValue::new("Peak at", time_of_day(self.peak.at)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uom::si::{
        f64::{Length, Time},
        length::millimeter,
        time::hour,
    };

    use super::*;
    use crate::{
        hover_tooltips::HoverTarget, widget_groups::weather_group::WeatherTarget,
        widgets::ongoing_toast::OngoingToast,
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
        RainForecast {
            rain,
            alerts: &RainAlertsConfig::default(),
        }
        .severity(clock.local())
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
    fn a_dry_forecast_stays_in_the_bar_as_inactive() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0]);
        let forecast = RainForecast {
            rain: &rain,
            alerts: &RainAlertsConfig::default(),
        };
        assert!(forecast.visible_in_bar());
        assert_eq!(forecast.state(clock), WidgetState::Off);
        assert_eq!(
            forecast.icon(),
            Glyph::from(Phosphor::DropSlash(Weight::Regular))
        );
        assert_eq!(
            RainForecast {
                rain: &radar_frames(clock, &[0.0, 0.2]),
                alerts: &RainAlertsConfig::default(),
            }
            .state(clock),
            WidgetState::Idle
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
        let body = RainForecast {
            rain: &rain,
            alerts: &RainAlertsConfig::default(),
        }
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
            Some("in 6m".to_owned())
        );
    }

    #[test]
    fn the_toast_body_stays_the_same_as_the_clock_advances() {
        let clock = Clock::real();
        let rain = radar_frames(clock, &[0.0, 0.0, 3.0]);
        let forecast = RainForecast {
            rain: &rain,
            alerts: &RainAlertsConfig::default(),
        };
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
        let toast = OngoingToast::from_widget_state(
            HoverTarget::Weather(WeatherTarget::Rain),
            &RainForecast {
                rain: &rain,
                alerts: &RainAlertsConfig::default(),
            },
            0_u8,
            Locale::default(),
            clock,
        );
        assert_eq!(toast.map(|toast| toast.severity), Some(Severity::Warning));
    }

    #[test]
    fn intensity_prints_in_millimeters_per_hour() {
        assert_eq!(
            intensity_text(Velocity::new::<millimeter_per_hour>(2.5)),
            "2.5 mm/h"
        );
    }
}
