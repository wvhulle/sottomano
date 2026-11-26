use chrono::{DateTime, Local, Locale};
use sottomano_desktop_sources::weather::buienradar_rain::{
    Downpour, Rain, RainOutlook, precipitation::millimeter_per_hour,
};
use sottomano_toml_config::{
    panel_menus::agenda_menu::rain_alerts::{DEFAULT_RAIN_ALERTS, RainAlertsConfig},
    severity::Severity,
};
use uom::si::f64::Velocity;

use super::{
    RainForecast, RainSources, current_intensity_text, intensity_text, outlook_turn,
    rain_strength_scale::RainIntensityScale,
};
use crate::{
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::indicator_state::IndicatorState,
    hover_cards::hover_card_body::Section,
    panel_menus::agenda_menu::weather_group::rain_forecast_ranking::frames_not_yet_over,
    widgets::{
        localized_durations::{duration_text, relative_moment_text},
        ongoing_toast::{definition_list, moment_text},
    },
};

pub(super) struct RadarRainForecast<'forecast> {
    pub(super) rain: &'forecast Rain,
    pub(super) alerts: &'forecast RainAlertsConfig,
}

impl<'forecast> RadarRainForecast<'forecast> {
    pub(super) fn with_default_alerts(rain: &'forecast Rain) -> Self {
        RadarRainForecast {
            rain,
            alerts: &DEFAULT_RAIN_ALERTS,
        }
    }
}

struct RainSpell<'forecast> {
    outlook: RainOutlook,
    starts_at: DateTime<Local>,
    peak: &'forecast Downpour,
}

impl RadarRainForecast<'_> {
    fn remaining_frames(&self, now: DateTime<Local>) -> &[Downpour] {
        frames_not_yet_over(&self.rain.forecast, now)
    }

    pub(super) fn intensity_scale(&self) -> RainIntensityScale {
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
        let outlook = RainOutlook::rain_above_threshold(remaining, minimum);
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

    pub(super) fn severity(&self, now: DateTime<Local>) -> Severity {
        let heavy = Velocity::new::<millimeter_per_hour>(self.alerts.heavy_millimeters_per_hour);
        match self.alerting_spell(now) {
            Some(spell) if spell.peak.intensity >= heavy => Severity::Critical,
            Some(_) => Severity::Warning,
            None => Severity::Normal,
        }
    }
}

impl RadarRainForecast<'_> {
    pub(super) fn glyph(&self) -> IconGlyph {
        self.intensity_scale()
            .strength(self.rain.peak.intensity)
            .glyph()
    }

    pub(super) fn display_state(&self, clock: Clock) -> IndicatorState {
        match self.severity(clock.local()) {
            Severity::Normal => rain_display_state(self.rain),
            alerting => alerting.into(),
        }
    }

    pub(super) fn toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
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

    pub(super) fn hover_card(&self, clock: Clock) -> Vec<Section> {
        RainForecast {
            sources: RainSources::Radar(self.rain),
            alerts: self.alerts,
        }
        .hover_card(clock)
    }
}

pub(super) fn rain_label_text(rain: &Rain, clock: Clock, locale: Locale) -> Option<String> {
    let remaining = |at: DateTime<Local>| duration_text(at - clock.local(), locale);
    match rain.outlook {
        RainOutlook::Dry => None,
        RainOutlook::StartsAt(at) => Some(relative_moment_text(at.to_utc(), clock.utc(), locale)),
        RainOutlook::StopsAt(at) => Some(format!("for {}", remaining(at))),
        RainOutlook::OutlastsForecastUntil(at) => Some(format!("for {}+", remaining(at))),
    }
}

pub(super) fn rain_display_state(rain: &Rain) -> IndicatorState {
    if rain.outlook == RainOutlook::Dry {
        IndicatorState::Off
    } else {
        IndicatorState::Idle
    }
}
