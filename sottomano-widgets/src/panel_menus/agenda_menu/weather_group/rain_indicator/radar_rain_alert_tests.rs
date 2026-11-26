use std::time::Duration;

use sottomano_desktop_sources::weather::buienradar_rain::Downpour;
use uom::si::{
    f64::{Length, Time},
    length::millimeter,
    time::hour,
};

use super::*;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
    panel_menus::agenda_menu::weather_group::{
        WeatherBusRequest, WeatherInteraction, WeatherSource,
        rain_indicator::rain_forecast_ranking::{RADAR_FRAME_LENGTH, frames_not_yet_over},
    },
};

#[test]
fn millimeter_per_hour_is_a_millimeter_each_hour() {
    let from_unit = Velocity::new::<millimeter_per_hour>(2.5);
    let from_parts = Length::new::<millimeter>(2.5) / Time::new::<hour>(1.0);
    assert!((from_unit - from_parts).abs() < Velocity::new::<millimeter_per_hour>(1e-9));
}

pub(super) fn radar_frames(clock: Clock, millimeters_per_hour: &[f64]) -> Rain {
    Rain::try_from(
        (0..)
            .zip(millimeters_per_hour)
            .map(|(frame, intensity)| Downpour {
                intensity: Velocity::new::<millimeter_per_hour>(*intensity),
                at: clock.local() + RADAR_FRAME_LENGTH * frame,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn radar_alert_severity(rain: &Rain, clock: Clock) -> Severity {
    RadarRainForecast::with_default_alerts(rain).severity(clock.local())
}

#[test]
fn a_dry_forecast_never_alerts() {
    let clock = Clock::real();
    assert_eq!(
        radar_alert_severity(&radar_frames(clock, &[0.0, 0.0]), clock),
        Severity::Normal
    );
}

#[test]
fn a_dry_forecast_stays_on_the_panel_as_inactive() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0]);
    let forecast = RadarRainForecast::with_default_alerts(&rain);
    assert_eq!(
        RainForecast {
            sources: RainSources::Radar(&rain),
            alerts: &DEFAULT_RAIN_ALERTS,
        }
        .indicator(clock, Locale::default())
        .panel_visibility,
        PanelVisibility::Shown
    );
    assert_eq!(forecast.display_state(clock), IndicatorState::Off);
    assert_eq!(
        forecast.glyph(),
        IconGlyph::from(Phosphor::DropSlash(Weight::Regular))
    );
    assert_eq!(
        RadarRainForecast::with_default_alerts(&radar_frames(clock, &[0.0, 0.2]))
            .display_state(clock),
        IndicatorState::Idle
    );
}

#[test]
fn rain_beyond_the_lead_time_waits_until_it_comes_closer() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0]);
    assert_eq!(radar_alert_severity(&rain, clock), Severity::Normal);
    assert_eq!(
        radar_alert_severity(&rain, clock.advanced(Duration::from_mins(30))),
        Severity::Warning
    );
}

#[test]
fn heavy_rain_within_the_lead_time_is_critical() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0, 12.0]);
    assert_eq!(radar_alert_severity(&rain, clock), Severity::Critical);
}

#[test]
fn drizzle_below_the_minimum_stays_quiet() {
    let clock = Clock::real();
    assert_eq!(
        radar_alert_severity(&radar_frames(clock, &[0.2, 0.3, 0.2, 0.0]), clock),
        Severity::Normal
    );
    assert_eq!(
        radar_alert_severity(&radar_frames(clock, &[2.0, 2.0, 0.0]), clock),
        Severity::Warning
    );
}

#[test]
fn drizzle_now_does_not_bring_a_far_shower_forward() {
    let clock = Clock::real();
    let mut frames = vec![0.2; 20];
    frames.push(12.0);
    assert_eq!(
        radar_alert_severity(&radar_frames(clock, &frames), clock),
        Severity::Normal
    );
}

#[test]
fn the_toast_describes_the_shower_rather_than_the_drizzle_before_it() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.2, 0.2, 3.0, 1.0, 0.0, 9.0]);
    let body = RadarRainForecast::with_default_alerts(&rain)
        .toast_body(clock, Locale::default())
        .unwrap();
    assert_eq!(radar_alert_severity(&rain, clock), Severity::Warning);
    assert!(body.contains("<dt>Starts</dt>"));
    assert!(body.contains("<dt>Peak</dt><dd>3.0 mm/h</dd>"));
}

#[test]
fn a_forecast_kept_through_failed_fetches_ages_out() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[3.0, 3.0, 3.0]);
    assert_eq!(radar_alert_severity(&rain, clock), Severity::Warning);
    assert_eq!(
        radar_alert_severity(&rain, clock.advanced(Duration::from_mins(15))),
        Severity::Normal
    );
}

#[test]
fn the_label_counts_down_with_the_clock() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0, 3.0]);
    let later = clock.advanced(Duration::from_mins(4));
    assert_eq!(
        rain_label_text(&rain, later, Locale::default()),
        Some("in 6 min.".to_owned())
    );
}

#[test]
fn the_toast_body_stays_the_same_as_the_clock_advances() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 0.0, 3.0]);
    let forecast = RadarRainForecast::with_default_alerts(&rain);
    let body = forecast.toast_body(clock, Locale::default());
    assert!(
        body.as_deref()
            .is_some_and(|body| body.contains("<dt>Starts</dt>"))
    );
    assert_eq!(
        forecast.toast_body(clock.advanced(Duration::from_secs(50)), Locale::default()),
        body
    );
}

#[test]
fn rain_starting_soon_raises_a_toast() {
    let clock = Clock::real();
    let rain = radar_frames(clock, &[0.0, 3.0]);
    let rain_indicator: GroupIndicator<'_> = RainForecast {
        sources: RainSources::Radar(&rain),
        alerts: &DEFAULT_RAIN_ALERTS,
    }
    .indicator(clock, Locale::default());
    let toast = rain_indicator
        .toast_requests(
            &IndicatorGroupIdentity::sole(IndicatorGroupFamily::Weather),
            WeatherInteraction::OnBuses(WeatherBusRequest::Refresh(WeatherSource::Rain)).into(),
            Locale::default(),
            clock,
        )
        .ongoing_toasts
        .into_iter()
        .next();
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
