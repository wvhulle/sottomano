use iced::{Subscription, Task};
use sottomano_toml_config::Config;

use crate::widget_groups::{
    weather_group::{
        geolocation_fix::{LOCATION_FALLBACK_GLYPH, location_hover_card, location_reading_row},
        pollen_count_gauge::{POLLEN_FALLBACK_GLYPH, allergen_hover_card, allergen_reading_row},
        rain_intensity::{
            PRECIPITATION_MODEL_FALLBACK_GLYPH, RAIN_FALLBACK_GLYPH,
            precipitation_model_hover_card, precipitation_model_reading_row, rain_hover_card,
            rain_reading_row,
        },
        temperature_forecast::{
            TEMPERATURE_FALLBACK_GLYPH, temperature_hover_card, temperature_reading_row,
        },
        wind_speed::{WIND_FALLBACK_GLYPH, wind_hover_card, wind_reading_row},
    },
    widget_group_identity::WidgetGroupFamily,
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
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        weather_group::rain_intensity::{RainForecast, RainSources},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

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

#[derive(Debug, Clone)]
pub enum WeatherInteraction {
    Expansion(WidgetGroupExpansionMessage),
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
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum WeatherMessage {
    Temperature(SnapshotDelivery<Temperature>),
    Wind(SnapshotDelivery<Wind>),
    Rain(SnapshotDelivery<Rain>),
    PrecipitationModel(SnapshotDelivery<PrecipitationModel>),
    Pollen(SnapshotDelivery<Pollen>),
    Location(SnapshotDelivery<Location>),
    Interaction(WeatherInteraction),
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
        let demand = demand.expanded_when(self.expansion.state());
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
            demand.while_rows_shown(
                snapshot_subscription::<Wind>(online_here()).map(WeatherMessage::Wind),
            ),
            demand.while_rows_shown(
                snapshot_subscription::<Pollen>(online_here()).map(WeatherMessage::Pollen),
            ),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage, clock: Clock) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temperature(reading) => self.temperature.receive(reading, clock),
            WeatherMessage::Wind(reading) => self.wind.receive(reading, clock),
            WeatherMessage::Rain(reading) => self.rain.receive(reading, clock),
            WeatherMessage::PrecipitationModel(reading) => {
                self.precipitation_model.receive(reading, clock);
            }
            WeatherMessage::Pollen(reading) => self.pollen.receive(reading, clock),
            WeatherMessage::Location(reading) => {
                self.location.receive(reading, clock);
            }
            WeatherMessage::Interaction(WeatherInteraction::Expansion(expansion_message)) => {
                self.expansion.update(expansion_message);
            }
            WeatherMessage::Interaction(WeatherInteraction::RefreshLocation) => {
                return first_delivery::<Location>(()).map(WeatherMessage::Location);
            }
            WeatherMessage::Interaction(WeatherInteraction::OnBuses(_)) => {}
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

    fn rain_forecast<'a>(&'a self, config: &'a Config) -> Option<RainForecast<'a>> {
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
            alerts: &config.widget_groups.weather.rain_alerts,
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

    fn rain_item<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Option<GroupItem<'a, WeatherInteraction>> {
        let locale = config.calendar.locale;
        let rain_item = match self.rain_forecast(config) {
            Some(forecast) => Some(GroupItem::reading(forecast.reading_row(clock, locale))),
            None => self
                .rain
                .group_item(
                    RAIN_FALLBACK_GLYPH,
                    |snapshot| rain_reading_row(snapshot, clock, locale),
                    clock,
                )
                .or_else(|| {
                    self.precipitation_model.group_item(
                        PRECIPITATION_MODEL_FALLBACK_GLYPH,
                        |snapshot| precipitation_model_reading_row(snapshot, clock, locale),
                        clock,
                    )
                }),
        };
        rain_item.map(|item| {
            item.with_hover_card(Box::new(move |card_clock| {
                self.rain_hover_card(config, card_clock)
            }))
            .refreshed_by(refresh_online(WeatherSource::Rain))
        })
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, WeatherInteraction> {
        let airborne_items = self.pollen.get().into_iter().flat_map(|pollen| {
            pollen.airborne().map(|concentration| {
                GroupItem::reading(allergen_reading_row(concentration))
                    .with_hover_card(Box::new(move |_| {
                        self.pollen
                            .hover_card_with(allergen_hover_card(concentration))
                    }))
                    .refreshed_by(refresh_online(WeatherSource::Pollen))
            })
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Weather,
            &self.expansion,
            WeatherInteraction::Expansion,
            [
                self.temperature
                    .group_item(
                        TEMPERATURE_FALLBACK_GLYPH,
                        |snapshot| temperature_reading_row(snapshot),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.temperature
                                .sections_described_by(temperature_hover_card)
                        }))
                        .refreshed_by(refresh_online(WeatherSource::Temperature))
                    }),
                self.rain_item(config, clock),
                self.wind
                    .group_item(
                        WIND_FALLBACK_GLYPH,
                        |snapshot| wind_reading_row(snapshot),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.wind.sections_described_by(wind_hover_card)
                        }))
                        .refreshed_by(refresh_online(WeatherSource::Wind))
                    }),
            ]
            .into_iter()
            .flatten()
            .chain(airborne_items)
            .chain(
                [
                    self.pollen.failure().map(|widget| {
                        GroupItem::reading(widget.reading_row(POLLEN_FALLBACK_GLYPH))
                            .with_hover_card(Box::new(move |_| {
                                self.pollen.sections_described_by(|_pollen| Vec::new())
                            }))
                            .refreshed_by(refresh_online(WeatherSource::Pollen))
                    }),
                    self.location
                        .group_item(
                            LOCATION_FALLBACK_GLYPH,
                            |snapshot| location_reading_row(snapshot),
                            clock,
                        )
                        .map(|item| {
                            item.with_hover_card(Box::new(move |_| {
                                self.location.sections_described_by(location_hover_card)
                            }))
                            .refreshed_by(WeatherInteraction::RefreshLocation)
                        }),
                ]
                .into_iter()
                .flatten(),
            ),
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
