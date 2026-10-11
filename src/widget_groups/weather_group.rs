use iced::{Subscription, Task};

pub mod geolocation_fix;
pub mod pollen_count_gauge;
pub mod rain_intensity;
pub mod temperature_forecast;
pub mod wind_speed;

use crate::{
    data::source::weather::{self, Allergen, Location, Pollen, Rain, Temperature, Wind},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
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
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WeatherMessage {
    Temp(Result<Temperature, weather::Error>),
    Wind(Result<Wind, weather::Error>),
    Rain(Result<Rain, weather::Error>),
    Pollen(Result<Pollen, weather::Error>),
    Location(Result<Location, weather::Error>),
    Expansion(ExpansionMessage),
}

impl WeatherGroup {
    pub fn subscription(&self) -> Subscription<WeatherMessage> {
        let location = Subscription::run(Location::published).map(WeatherMessage::Location);
        let Some(here) = self.location.get().map(Location::position) else {
            return location;
        };
        Subscription::batch([
            location,
            polling::<Temperature>(here).map(WeatherMessage::Temp),
            polling::<Wind>(here).map(WeatherMessage::Wind),
            polling::<Rain>(here).map(WeatherMessage::Rain),
            polling::<Pollen>(here).map(WeatherMessage::Pollen),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temp(reading) => self.temp.receive(reading),
            WeatherMessage::Wind(reading) => self.wind.receive(reading),
            WeatherMessage::Rain(reading) => self.rain.receive(reading),
            WeatherMessage::Pollen(reading) => self.pollen.receive(reading),
            WeatherMessage::Location(reading) => self.location.receive(reading),
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
            WeatherTarget::Location => self.location.live_tooltip(),
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
                self.rain.present_group_item(WeatherTarget::Rain),
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
