use iced::{Subscription, Task};
use sottomano_toml_config::Config;

use crate::widget_groups::widget_group_identity::WidgetGroupFamily;

mod geolocation_fix;
mod pollen_count_gauge;
mod precipitation_type_forecast;
mod rain_intensity;
mod temperature_forecast;
mod wind_speed;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    weather::{
        self,
        buienradar_rain::Rain,
        open_meteo_pollen::{Allergen, Pollen},
        open_meteo_precipitation_model::PrecipitationModel,
        open_meteo_temperature::Temperature,
        open_meteo_wind::Wind,
        situo_location_fix::Location,
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        weather_group::rain_intensity::{RainForecast, RainSources},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::latest_snapshot::{HidesFailuresAfterSuccess, LatestSnapshot},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherTarget {
    Temperature,
    Wind,
    Rain,
    Pollen,
    Allergen(Allergen),
    Location,
}

impl From<weather::Error> for Section {
    fn from(error: weather::Error) -> Self {
        match error {
            weather::Error::Fetch(fetch) => fetch.into(),
            other => Section::error(other),
        }
    }
}

impl From<WeatherTarget> for HoverTarget {
    fn from(target: WeatherTarget) -> Self {
        HoverTarget::Weather(target)
    }
}

#[derive(Default)]
pub struct WeatherGroup {
    temperature: LatestSnapshot<Temperature>,
    wind: LatestSnapshot<Wind>,
    rain: LatestSnapshot<Rain, HidesFailuresAfterSuccess>,
    precipitation_model: LatestSnapshot<PrecipitationModel, HidesFailuresAfterSuccess>,
    pollen: LatestSnapshot<Pollen>,
    location: LatestSnapshot<Location, HidesFailuresAfterSuccess>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WeatherMessage {
    Temperature(SnapshotDelivery<Temperature>),
    Wind(SnapshotDelivery<Wind>),
    Rain(SnapshotDelivery<Rain>),
    PrecipitationModel(SnapshotDelivery<PrecipitationModel>),
    Pollen(SnapshotDelivery<Pollen>),
    Location(SnapshotDelivery<Location>),
    Expansion(WidgetGroupExpansionMessage),
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
        let online_here = || OnlineSourceInputs {
            buses: buses.clone(),
            source_inputs: here,
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
            WeatherMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn refresh(
        &self,
        target: WeatherTarget,
        buses: Option<&DesktopBuses>,
    ) -> Task<WeatherMessage> {
        let Some(here) = self.location.get().map(Location::position) else {
            return first_delivery::<Location>(()).map(WeatherMessage::Location);
        };
        if let WeatherTarget::Location = target {
            return first_delivery::<Location>(()).map(WeatherMessage::Location);
        }
        let Some(buses) = buses else {
            return Task::none();
        };
        let online_here = || OnlineSourceInputs {
            buses: buses.clone(),
            source_inputs: here,
        };
        match target {
            WeatherTarget::Temperature => {
                first_delivery::<Temperature>(online_here()).map(WeatherMessage::Temperature)
            }
            WeatherTarget::Wind => first_delivery::<Wind>(online_here()).map(WeatherMessage::Wind),
            WeatherTarget::Rain => Task::batch([
                first_delivery::<Rain>(online_here()).map(WeatherMessage::Rain),
                first_delivery::<PrecipitationModel>(online_here())
                    .map(WeatherMessage::PrecipitationModel),
            ]),
            WeatherTarget::Pollen | WeatherTarget::Allergen(_) => {
                first_delivery::<Pollen>(online_here()).map(WeatherMessage::Pollen)
            }
            WeatherTarget::Location => Task::none(),
        }
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: WeatherTarget,
        config: &Config,
        clock: Clock,
    ) -> Option<HoverCard> {
        match target {
            WeatherTarget::Temperature => self.temperature.hover_card(clock),
            WeatherTarget::Wind => self.wind.hover_card(clock),
            WeatherTarget::Rain => self.rain_hover_card(config, clock),
            WeatherTarget::Pollen => self.pollen.hover_card(clock),
            WeatherTarget::Allergen(allergen) => self.pollen.hover_card_with(
                self.pollen
                    .get()?
                    .concentration(allergen)?
                    .hover_card(clock),
            ),
            WeatherTarget::Location => self.location.hover_card(clock),
        }
    }

    fn rain_forecast<'a>(&'a self, config: &'a Config) -> Option<RainForecast<'a>> {
        let sources = match (
            self.rain.visible_reading(),
            self.precipitation_model.visible_reading(),
        ) {
            (Some(radar), Some(model)) => RainSources::RadarAndModel { radar, model },
            (Some(radar), None) => RainSources::Radar(radar),
            (None, Some(model)) => RainSources::Model(model),
            (None, None) => return None,
        };
        Some(RainForecast {
            sources,
            alerts: &config.widget_groups.weather.rain_alerts,
        })
    }

    fn rain_hover_card(&self, config: &Config, clock: Clock) -> Option<HoverCard> {
        let Some(forecast) = self.rain_forecast(config) else {
            return self
                .rain
                .hover_card(clock)
                .or_else(|| self.precipitation_model.hover_card(clock));
        };
        let radar_failure_sections = self
            .rain
            .failure()
            .map(|failure| failure.hover_card(clock).listed_sections().to_vec())
            .unwrap_or_default();
        let body = forecast
            .hover_card(clock)
            .with_sections(radar_failure_sections);
        match forecast.sources {
            RainSources::Model(_) => self.precipitation_model.hover_card_with(body),
            RainSources::RadarAndModel { .. } | RainSources::Radar(_) => {
                self.rain.hover_card_with(body)
            }
        }
    }

    fn rain_item<'a>(&'a self, config: &'a Config) -> Option<GroupItem<'a, WeatherMessage>> {
        match self.rain_forecast(config) {
            Some(forecast) => Some(GroupItem::reading(forecast, WeatherTarget::Rain)),
            None => self
                .rain
                .group_item(WeatherTarget::Rain)
                .or_else(|| self.precipitation_model.group_item(WeatherTarget::Rain)),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, WeatherMessage> {
        let airborne_items = self.pollen.get().into_iter().flat_map(|pollen| {
            pollen.airborne().map(|concentration| {
                GroupItem::reading(
                    concentration as &dyn GroupItemReading,
                    WeatherTarget::Allergen(concentration.allergen),
                )
            })
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Weather,
            &self.expansion,
            WeatherMessage::Expansion,
            [
                self.temperature.group_item(WeatherTarget::Temperature),
                self.rain_item(config),
                self.wind.group_item(WeatherTarget::Wind),
            ]
            .into_iter()
            .flatten()
            .chain(airborne_items)
            .chain(
                [
                    self.pollen
                        .failure()
                        .map(|widget| GroupItem::reading(widget, WeatherTarget::Pollen)),
                    self.location.group_item(WeatherTarget::Location),
                ]
                .into_iter()
                .flatten(),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        weather::buienradar_rain::{Downpour, precipitation::millimeter_per_hour},
    };
    use uom::si::f64::Velocity;

    use super::*;

    const RAIN_CADENCE: DeliveryCadence =
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(60)));

    fn delivered_rain(clock: Clock) -> SnapshotDelivery<Rain> {
        SnapshotDelivery::fixture_delivered(
            Ok(Rain::from_forecast(vec![Downpour {
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
