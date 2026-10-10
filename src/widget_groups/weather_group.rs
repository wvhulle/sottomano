use iced::{Subscription, Task};

pub mod geolocation_fix;
pub mod pollen_count_gauge;
pub mod rain_intensity;
pub mod temperature_forecast;
pub mod wind_speed;

use crate::{
    data::source::weather::{self, Location, Pollen, Rain, Temperature, Wind},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Default)]
pub struct WeatherGroup {
    temp: LastPoll<Temperature>,
    wind: LastPoll<Wind>,
    rain: LastPoll<Rain>,
    pollen: LastPoll<Pollen>,
    location: LastPoll<Location>,
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::WeatherTemp => self.temp.tooltip(),
            HoverTarget::WeatherWind => self.wind.tooltip(),
            HoverTarget::WeatherRain => self.rain.tooltip(),
            HoverTarget::Pollen => self.pollen.tooltip(),
            HoverTarget::Allergen(allergen) => self
                .pollen
                .tooltip_with(self.pollen.get()?.concentration(*allergen)?.tooltip()),
            HoverTarget::Location => self.location.live_tooltip(),
            _ => None,
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, WeatherMessage> {
        let airborne_items = self.pollen.get().into_iter().flat_map(|pollen| {
            pollen.airborne().map(|concentration| {
                GroupItem::reading(
                    concentration as &dyn BarWidget,
                    HoverTarget::Allergen(concentration.allergen),
                )
            })
        });
        GroupContent::new(
            &self.expansion,
            WeatherMessage::Expansion,
            [
                self.temp.group_item(HoverTarget::WeatherTemp),
                self.wind.group_item(HoverTarget::WeatherWind),
                self.rain.present_group_item(HoverTarget::WeatherRain),
            ]
            .into_iter()
            .flatten()
            .chain(airborne_items)
            .chain(
                [
                    self.pollen
                        .failure()
                        .map(|widget| GroupItem::reading(widget, HoverTarget::Pollen)),
                    self.location.group_item(HoverTarget::Location),
                ]
                .into_iter()
                .flatten(),
            ),
        )
    }
}
