use iced::{Subscription, Task};
use sottomano_toml_config::Config;

use crate::{
    group_indicators::{
        indicator_group_content::IndicatorListing, indicator_group_identity::IndicatorGroupFamily,
    },
    panel_menus::agenda_menu::weather_group::temperature_indicator::temperature_indicator,
};

mod pollen_indicator;
#[cfg(test)]
mod rain_fetch_failure_tests;
mod rain_indicator;
mod temperature_indicator;
mod wind_indicator;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
    weather::{
        buienradar_rain::Rain,
        open_meteo_pollen::Pollen,
        open_meteo_precipitation_model::PrecipitationModel,
        open_meteo_temperature::Temperature,
        open_meteo_wind::Wind,
        situo_location_fix::{ForecastGrid, Location},
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
    },
    group_menus::group_source_demand::GroupSourceDemand,
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
        let (Some(here), Some(buses)) = (self.location.get(), buses) else {
            return location;
        };
        let online_on = |forecast_grid| OnBus {
            buses: buses.clone(),
            inputs: here.position_on(forecast_grid),
        };
        Subscription::batch([
            location,
            snapshot_subscription::<Temperature>(online_on(ForecastGrid::OpenMeteo))
                .map(WeatherMessage::Temperature),
            snapshot_subscription::<Rain>(online_on(ForecastGrid::Buienradar))
                .map(WeatherMessage::Rain),
            snapshot_subscription::<PrecipitationModel>(online_on(ForecastGrid::OpenMeteo))
                .map(WeatherMessage::PrecipitationModel),
            demand.while_indicators_shown(
                snapshot_subscription::<Wind>(online_on(ForecastGrid::OpenMeteo))
                    .map(WeatherMessage::Wind),
            ),
            demand.while_indicators_shown(
                snapshot_subscription::<Pollen>(online_on(ForecastGrid::OpenMeteo))
                    .map(WeatherMessage::Pollen),
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
        let Some(here) = self.location.get() else {
            return first_delivery::<Location>(()).map(WeatherMessage::Location);
        };
        let online_on = |forecast_grid| OnBus {
            buses: buses.clone(),
            inputs: here.position_on(forecast_grid),
        };
        match source {
            WeatherSource::Temperature => {
                first_delivery::<Temperature>(online_on(ForecastGrid::OpenMeteo))
                    .map(WeatherMessage::Temperature)
            }
            WeatherSource::Wind => {
                first_delivery::<Wind>(online_on(ForecastGrid::OpenMeteo)).map(WeatherMessage::Wind)
            }
            WeatherSource::Rain => Task::batch([
                first_delivery::<Rain>(online_on(ForecastGrid::Buienradar))
                    .map(WeatherMessage::Rain),
                first_delivery::<PrecipitationModel>(online_on(ForecastGrid::OpenMeteo))
                    .map(WeatherMessage::PrecipitationModel),
            ]),
            WeatherSource::Pollen => first_delivery::<Pollen>(online_on(ForecastGrid::OpenMeteo))
                .map(WeatherMessage::Pollen),
        }
    }

    fn temperature_headline(&self) -> GroupIndicator<'_> {
        self.wired_temperature_indicator(self.temperature.indicator(
            |held_snapshot| temperature_indicator(held_snapshot),
            WEATHER_GLYPH.into(),
        ))
        .with_headline_glyph(Phosphor::Umbrella(Weight::Regular).into())
    }

    fn weather_listing<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorListing<'view> {
        if self.location.get().is_none() {
            return self
                .location
                .indicator_listing(|_| Vec::new(), WEATHER_GLYPH.into());
        }
        [
            self.temperature_listing(),
            self.rain_listing(config, clock),
            self.wind_listing(),
            self.pollen_listing(),
        ]
        .into_iter()
        .collect()
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'view> {
        let headline_indicator = self.temperature_headline();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Weather,
            headline_indicator,
            &self.expansion,
            WeatherInteraction::Expansion,
            self.weather_listing(config, clock),
        )
    }
}

fn refresh_online(source: WeatherSource) -> WeatherInteraction {
    WeatherInteraction::OnBuses(WeatherBusRequest::Refresh(source))
}
