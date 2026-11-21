use iced::{Subscription, Task};
use sottomano_toml_config::Config;

use crate::indicator_groups::{
    indicator_group_identity::IndicatorGroupFamily,
    weather_group::{
        geolocation_fix::{LOCATION_FALLBACK_GLYPH, location_hover_card, location_indicator},
        pollen_count_gauge::{POLLEN_FALLBACK_GLYPH, allergen_hover_card, allergen_indicator},
        rain_intensity::{
            PRECIPITATION_MODEL_FALLBACK_GLYPH, RAIN_FALLBACK_GLYPH,
            precipitation_model_hover_card, rain_hover_card,
        },
        temperature_forecast::{temperature_hover_card, temperature_indicator},
        wind_speed::{WIND_FALLBACK_GLYPH, wind_hover_card, wind_indicator},
    },
};

mod geolocation_fix;
mod pollen_count_gauge;
mod precipitation_type_forecast;
mod rain_forecast_ranking;
mod rain_intensity;
mod temperature_forecast;
mod wind_speed;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
    weather::{
        buienradar_rain::Rain, open_meteo_pollen::Pollen,
        open_meteo_precipitation_model::PrecipitationModel, open_meteo_temperature::Temperature,
        open_meteo_wind::Wind, situo_location_fix::Location,
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        weather_group::rain_intensity::{RainForecast, RainSources},
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const WEATHER_GLYPH: Phosphor = Phosphor::CloudSun(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherSource {
    Temperature,
    Wind,
    Rain,
    Pollen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherBusRequest {
    Refresh(WeatherSource),
}

#[derive(Debug, Clone, Copy)]
pub enum WeatherInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    RefreshLocation,
    OnBuses(WeatherBusRequest),
}

#[derive(Default)]
pub struct WeatherGroup {
    temperature: SubscribedSnapshot<Temperature>,
    wind: SubscribedSnapshot<Wind>,
    rain: SubscribedSnapshot<Rain>,
    precipitation_model: SubscribedSnapshot<PrecipitationModel>,
    pollen: SubscribedSnapshot<Pollen>,
    location: SubscribedSnapshot<Location>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum WeatherMessage {
    Temperature(SnapshotDelivery<Temperature>),
    Wind(SnapshotDelivery<Wind>),
    Rain(SnapshotDelivery<Rain>),
    PrecipitationModel(SnapshotDelivery<PrecipitationModel>),
    Pollen(SnapshotDelivery<Pollen>),
    Location(SnapshotDelivery<Location>),
}

impl WeatherGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: Option<&DesktopBuses>,
        demand: GroupSourceDemand,
    ) -> Subscription<WeatherMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        let location = snapshot_subscription::<Location>(()).map(WeatherMessage::Location);
        let (Some(here), Some(buses)) = (self.location.get().map(Location::position), buses) else {
            return location;
        };
        let online_here = || OnBus {
            buses: buses.clone(),
            inputs: here,
        };
        Subscription::batch([
            location,
            snapshot_subscription::<Temperature>(online_here()).map(WeatherMessage::Temperature),
            snapshot_subscription::<Rain>(online_here()).map(WeatherMessage::Rain),
            snapshot_subscription::<PrecipitationModel>(online_here())
                .map(WeatherMessage::PrecipitationModel),
            demand.while_indicators_shown(
                snapshot_subscription::<Wind>(online_here()).map(WeatherMessage::Wind),
            ),
            demand.while_indicators_shown(
                snapshot_subscription::<Pollen>(online_here()).map(WeatherMessage::Pollen),
            ),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage, clock: Clock) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temperature(sample) => self.temperature.receive(sample, clock),
            WeatherMessage::Wind(sample) => self.wind.receive(sample, clock),
            WeatherMessage::Rain(sample) => self.rain.receive(sample, clock),
            WeatherMessage::PrecipitationModel(sample) => {
                self.precipitation_model.receive(sample, clock);
            }
            WeatherMessage::Pollen(sample) => self.pollen.receive(sample, clock),
            WeatherMessage::Location(sample) => {
                self.location.receive(sample, clock);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: WeatherInteraction) -> Task<WeatherMessage> {
        match interaction {
            WeatherInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            WeatherInteraction::RefreshLocation => {
                return first_delivery::<Location>(()).map(WeatherMessage::Location);
            }
            WeatherInteraction::OnBuses(_) => {}
        }
        Task::none()
    }

    pub fn on_buses(
        &self,
        request: WeatherBusRequest,
        buses: &DesktopBuses,
    ) -> Task<WeatherMessage> {
        let WeatherBusRequest::Refresh(source) = request;
        let Some(here) = self.location.get().map(Location::position) else {
            return first_delivery::<Location>(()).map(WeatherMessage::Location);
        };
        let online_here = || OnBus {
            buses: buses.clone(),
            inputs: here,
        };
        match source {
            WeatherSource::Temperature => {
                first_delivery::<Temperature>(online_here()).map(WeatherMessage::Temperature)
            }
            WeatherSource::Wind => first_delivery::<Wind>(online_here()).map(WeatherMessage::Wind),
            WeatherSource::Rain => Task::batch([
                first_delivery::<Rain>(online_here()).map(WeatherMessage::Rain),
                first_delivery::<PrecipitationModel>(online_here())
                    .map(WeatherMessage::PrecipitationModel),
            ]),
            WeatherSource::Pollen => {
                first_delivery::<Pollen>(online_here()).map(WeatherMessage::Pollen)
            }
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
            alerts: &config.indicator_groups.weather.rain_alerts,
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

    fn rain_indicator<'view>(
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

    fn wired_temperature_indicator<'view>(
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

    fn wired_wind_indicator<'view>(
        &'view self,
        wind_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        wind_row
            .with_hover_card(Box::new(move |_| {
                self.wind.sections_described_by(wind_hover_card)
            }))
            .refreshed_by(refresh_online(WeatherSource::Wind))
    }

    fn wired_location_indicator<'view>(
        &'view self,
        location_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        location_row
            .with_hover_card(Box::new(move |_| {
                self.location.sections_described_by(location_hover_card)
            }))
            .refreshed_by(WeatherInteraction::RefreshLocation)
    }

    fn temperature_headline(&self) -> GroupIndicator<'_> {
        self.wired_temperature_indicator(self.temperature.indicator(
            |held_snapshot| temperature_indicator(held_snapshot),
            WEATHER_GLYPH.into(),
        ))
        .with_headline_glyph(Phosphor::Umbrella(Weight::Regular).into())
    }

    fn list_temperature<'view>(&'view self, indicators: &mut Vec<GroupIndicator<'view>>) {
        if !self.temperature.awaits_first_delivery() {
            indicators.insert(
                0,
                self.wired_temperature_indicator(self.temperature.indicator(
                    |held_snapshot| temperature_indicator(held_snapshot),
                    WEATHER_GLYPH.into(),
                )),
            );
        }
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'view> {
        let locale = config.calendar.locale;
        let headline_indicator = self.temperature_headline();
        let mut rows = Vec::new();
        if let Some(forecast) = self.rain_forecast(config) {
            rows.push(self.rain_indicator(config, forecast.indicator(clock, locale)));
        } else if let Some(rain_placeholder) = self.rain.failure() {
            rows.push(self.rain_indicator(config, rain_placeholder.indicator(RAIN_FALLBACK_GLYPH)));
        } else if let Some(model_placeholder) = self.precipitation_model.failure() {
            rows.push(self.rain_indicator(
                config,
                model_placeholder.indicator(PRECIPITATION_MODEL_FALLBACK_GLYPH),
            ));
        }
        if !self.wind.awaits_first_delivery() {
            let wind_row = self.wind.indicator(
                |held_snapshot| wind_indicator(held_snapshot),
                WIND_FALLBACK_GLYPH,
            );
            if wind_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_wind_indicator(wind_row));
            }
        }
        if let Some(pollen) = self.pollen.get() {
            rows.extend(pollen.airborne().map(|concentration| {
                allergen_indicator(concentration)
                    .with_hover_card(Box::new(move |_| {
                        self.pollen
                            .hover_card_with(allergen_hover_card(concentration))
                    }))
                    .refreshed_by(refresh_online(WeatherSource::Pollen))
            }));
        }
        if let Some(pollen_placeholder) = self.pollen.failure() {
            rows.push(
                pollen_placeholder
                    .indicator(POLLEN_FALLBACK_GLYPH)
                    .with_hover_card(Box::new(move |_| {
                        self.pollen.sections_described_by(|_pollen| Vec::new())
                    }))
                    .refreshed_by(refresh_online(WeatherSource::Pollen)),
            );
        }
        if !self.location.awaits_first_delivery() {
            let location_row = self.location.indicator(
                |held_snapshot| location_indicator(held_snapshot),
                LOCATION_FALLBACK_GLYPH,
            );
            if location_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_location_indicator(location_row));
            }
        }
        self.list_temperature(&mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Weather,
            headline_indicator,
            &self.expansion,
            WeatherInteraction::Expansion,
            rows,
        )
    }
}

fn refresh_online(source: WeatherSource) -> WeatherInteraction {
    WeatherInteraction::OnBuses(WeatherBusRequest::Refresh(source))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        weather::{
            self,
            buienradar_rain::{Downpour, precipitation::millimeter_per_hour},
        },
    };
    use uom::si::f64::Velocity;

    use super::*;

    const RAIN_CADENCE: DeliveryCadence =
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(60)));

    fn delivered_rain(clock: Clock) -> SnapshotDelivery<Rain> {
        SnapshotDelivery::fixture_delivered(
            Ok(Rain::try_from(vec![Downpour {
                intensity: Velocity::new::<millimeter_per_hour>(3.0),
                at: clock.local(),
            }])
            .unwrap()),
            RAIN_CADENCE,
        )
    }

    fn failed_rain(error: weather::Error) -> SnapshotDelivery<Rain> {
        SnapshotDelivery::fixture_delivered(Err(error), RAIN_CADENCE)
    }

    #[test]
    fn a_failed_rain_fetch_keeps_the_last_forecast() {
        let clock = Clock::real();
        let mut group = WeatherGroup::default();
        let _ = group.update(WeatherMessage::Rain(delivered_rain(clock)), clock);
        let _ = group.update(
            WeatherMessage::Rain(failed_rain(weather::Error::Malformed)),
            clock,
        );
        assert!(group.rain.get().is_some());
    }

    #[test]
    fn a_rain_failure_without_an_earlier_forecast_shows_as_failed() {
        let clock = Clock::real();
        let mut group = WeatherGroup::default();
        let _ = group.update(
            WeatherMessage::Rain(failed_rain(weather::Error::OutsideRainRadar)),
            clock,
        );
        assert!(group.rain.failure().is_some());
    }
}
