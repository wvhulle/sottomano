use sottomano_toml_config::Config;

use super::{
    PRECIPITATION_MODEL_FALLBACK_GLYPH, RAIN_FALLBACK_GLYPH, RainForecast, RainSources,
    precipitation_model_hover_card, rain_hover_card,
};
use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator, indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::Section,
    panel_menus::agenda_menu::weather_group::{WeatherGroup, WeatherSource, refresh_online},
};

impl WeatherGroup {
    pub(in crate::panel_menus::agenda_menu::weather_group) fn rain_listing<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorListing<'view> {
        if let Some(forecast) = self.rain_forecast(config) {
            return IndicatorListing::Listed(vec![
                self.wired_rain_indicator(
                    config,
                    forecast.indicator(clock, config.calendar.locale),
                ),
            ]);
        }
        let unread_rain = if let Some(rain_failure) = self.rain.failure() {
            rain_failure.indicator(RAIN_FALLBACK_GLYPH)
        } else if let Some(model_failure) = self.precipitation_model.failure() {
            model_failure.indicator(PRECIPITATION_MODEL_FALLBACK_GLYPH)
        } else {
            return IndicatorListing::Listed(Vec::new());
        };
        IndicatorListing::Incomplete {
            listed_indicators: Vec::new(),
            unread_sources: vec![self.wired_rain_indicator(config, unread_rain)],
        }
    }

    fn rain_forecast<'forecast>(
        &'forecast self,
        config: &'forecast Config,
    ) -> Option<RainForecast<'forecast>> {
        let visible_radar = self.rain.get();
        let visible_model = self.precipitation_model.get();
        let sources = match visible_radar {
            Some(radar) => match visible_model {
                Some(model) => RainSources::RadarAndModel { radar, model },
                None => RainSources::Radar(radar),
            },
            None => RainSources::Model(visible_model?),
        };
        Some(RainForecast {
            sources,
            alerts: &config.panel_menus.agenda.weather.rain_alerts,
        })
    }

    fn rain_hover_card(&self, config: &Config, clock: Clock) -> Vec<Section> {
        let Some(forecast) = self.rain_forecast(config) else {
            let radar_sections = self
                .rain
                .sections_described_by(|snapshot| rain_hover_card(snapshot, clock));
            if radar_sections.is_empty() {
                return self.precipitation_model.sections_described_by(|snapshot| {
                    precipitation_model_hover_card(snapshot, clock)
                });
            }
            return radar_sections;
        };
        let forecast_sections = forecast.hover_card(clock);
        let mut sections = match forecast.sources {
            RainSources::Model(_) => self.precipitation_model.hover_card_with(forecast_sections),
            RainSources::RadarAndModel { .. } | RainSources::Radar(_) => {
                self.rain.hover_card_with(forecast_sections)
            }
        };
        if self.rain.failure().is_some() {
            sections.extend(self.rain.hover_card_with(Vec::new()));
        }
        sections
    }

    fn wired_rain_indicator<'view>(
        &'view self,
        config: &'view Config,
        rain_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        rain_row
            .with_hover_card(Box::new(move |card_clock| {
                self.rain_hover_card(config, card_clock)
            }))
            .refreshed_by(refresh_online(WeatherSource::Rain))
    }
}
