use iced::{Subscription, Task};
use sottomano_config::Config;

mod geolocation_fix;
mod pollen_count_gauge;
mod rain_intensity;
mod temperature_forecast;
mod wind_speed;

use sottomano_sources::{
    SourceDelivery,
    weather::{
        self,
        buienradar_rain::Rain,
        open_meteo_pollen::{Allergen, Pollen},
        open_meteo_temperature::Temperature,
        open_meteo_wind::Wind,
        situo_location_fix::Location,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::weather_group::rain_intensity::RainForecast,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        tooltip_body::{Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
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
    temp: ContinuousReading<Temperature>,
    wind: ContinuousReading<Wind>,
    rain: ContinuousReading<Rain>,
    pollen: ContinuousReading<Pollen>,
    location: ContinuousReading<Location>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WeatherMessage {
    Temp(SourceDelivery<Temperature>),
    Wind(SourceDelivery<Wind>),
    Rain(SourceDelivery<Rain>),
    Pollen(SourceDelivery<Pollen>),
    Location(SourceDelivery<Location>),
    Expansion(ExpansionMessage),
}

impl WeatherGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self) -> Subscription<WeatherMessage> {
        let location = source_subscription::<Location>(()).map(WeatherMessage::Location);
        let Some(here) = self.location.get().map(Location::position) else {
            return location;
        };
        Subscription::batch([
            location,
            source_subscription::<Temperature>(here).map(WeatherMessage::Temp),
            source_subscription::<Rain>(here).map(WeatherMessage::Rain),
            self.expansion
                .while_expanded(source_subscription::<Wind>(here).map(WeatherMessage::Wind)),
            self.expansion
                .while_expanded(source_subscription::<Pollen>(here).map(WeatherMessage::Pollen)),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage, clock: Clock) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temp(reading) => self.temp.receive_delivery(reading, clock),
            WeatherMessage::Wind(reading) => self.wind.receive_delivery(reading, clock),
            WeatherMessage::Rain(reading) => self.rain.receive_keeping_last_success(reading, clock),
            WeatherMessage::Pollen(reading) => self.pollen.receive_delivery(reading, clock),
            WeatherMessage::Location(reading) => {
                self.location.receive_keeping_last_success(reading, clock);
            }
            WeatherMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn refresh(&self, target: WeatherTarget) -> Task<WeatherMessage> {
        let Some(here) = self.location.get().map(Location::position) else {
            return first_delivery::<Location>(()).map(WeatherMessage::Location);
        };
        match target {
            WeatherTarget::Temperature => {
                first_delivery::<Temperature>(here).map(WeatherMessage::Temp)
            }
            WeatherTarget::Wind => first_delivery::<Wind>(here).map(WeatherMessage::Wind),
            WeatherTarget::Rain => first_delivery::<Rain>(here).map(WeatherMessage::Rain),
            WeatherTarget::Pollen | WeatherTarget::Allergen(_) => {
                first_delivery::<Pollen>(here).map(WeatherMessage::Pollen)
            }
            WeatherTarget::Location => first_delivery::<Location>(()).map(WeatherMessage::Location),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: WeatherTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            WeatherTarget::Temperature => self.temp.tooltip(clock),
            WeatherTarget::Wind => self.wind.tooltip(clock),
            WeatherTarget::Rain => self.rain.tooltip(clock),
            WeatherTarget::Pollen => self.pollen.tooltip(clock),
            WeatherTarget::Allergen(allergen) => self
                .pollen
                .tooltip_with(self.pollen.get()?.concentration(allergen)?.tooltip(clock)),
            WeatherTarget::Location => self.location.tooltip(clock),
        }
    }

    fn rain_item<'a>(&'a self, config: &'a Config) -> Option<GroupItem<'a, WeatherMessage>> {
        let Some(rain) = self.rain.visible_reading() else {
            return self.rain.group_item(WeatherTarget::Rain);
        };
        Some(GroupItem::reading(
            RainForecast {
                rain,
                alerts: &config.widget_groups.rain_alerts,
            },
            WeatherTarget::Rain,
        ))
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, WeatherMessage> {
        let airborne_items = self.pollen.get().into_iter().flat_map(|pollen| {
            pollen.airborne().map(|concentration| {
                GroupItem::reading(
                    concentration as &dyn BarWidget,
                    WeatherTarget::Allergen(concentration.allergen),
                )
            })
        });
        GroupContent::new(
            &self.expansion,
            WeatherMessage::Expansion,
            [
                self.temp.group_item(WeatherTarget::Temperature),
                self.wind.group_item(WeatherTarget::Wind),
                self.rain_item(config),
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
    use sottomano_sources::weather::buienradar_rain::{
        Downpour, precipitation::millimeter_per_hour,
    };
    use uom::si::f64::Velocity;

    use super::*;

    fn delivered_rain(clock: Clock) -> SourceDelivery<Rain> {
        SourceDelivery::Reading(Ok(Rain::from_forecast(vec![Downpour {
            intensity: Velocity::new::<millimeter_per_hour>(3.0),
            at: clock.local(),
        }])
        .unwrap()))
    }

    fn failed_rain(error: weather::Error) -> SourceDelivery<Rain> {
        SourceDelivery::Reading(Err(error))
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
