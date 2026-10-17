use iced::{Subscription, Task};

mod geolocation_fix;
mod pollen_count_gauge;
mod rain_intensity;
mod temperature_forecast;
mod wind_speed;

use crate::{
    data::source::{
        SourceDelivery,
        weather::{Allergen, Location, Pollen, Rain, Temperature, Wind},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, source_subscription},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        tooltip_body::Tooltip,
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

    pub fn subscription(&self, forced: Option<ForcedRefresh>) -> Subscription<WeatherMessage> {
        let location = source_subscription::<Location>((), None).map(WeatherMessage::Location);
        let Some(here) = self.location.get().map(Location::position) else {
            return location;
        };
        Subscription::batch([
            location,
            source_subscription::<Temperature>(here, forced).map(WeatherMessage::Temp),
            self.expansion.while_expanded(
                source_subscription::<Wind>(here, forced).map(WeatherMessage::Wind),
            ),
            self.expansion.while_expanded(
                source_subscription::<Rain>(here, forced).map(WeatherMessage::Rain),
            ),
            self.expansion.while_expanded(
                source_subscription::<Pollen>(here, forced).map(WeatherMessage::Pollen),
            ),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temp(reading) => self.temp.receive_delivery(reading),
            WeatherMessage::Wind(reading) => self.wind.receive_delivery(reading),
            WeatherMessage::Rain(reading) => self.rain.receive_delivery(reading),
            WeatherMessage::Pollen(reading) => self.pollen.receive_delivery(reading),
            WeatherMessage::Location(reading) => self.location.receive_delivery(reading),
            WeatherMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: WeatherTarget) -> Option<Tooltip> {
        match target {
            WeatherTarget::Temperature => self.temp.tooltip(),
            WeatherTarget::Wind => self.wind.tooltip(),
            WeatherTarget::Rain => self.rain.tooltip(),
            WeatherTarget::Pollen => self.pollen.tooltip(),
            WeatherTarget::Allergen(allergen) => self
                .pollen
                .tooltip_with(self.pollen.get()?.concentration(allergen)?.tooltip()),
            WeatherTarget::Location => self.location.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, WeatherMessage> {
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
                self.rain.group_item(WeatherTarget::Rain),
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
