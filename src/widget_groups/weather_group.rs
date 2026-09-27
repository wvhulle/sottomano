use iced::{Element, Subscription, Task, widget::Row};

pub mod geolocation_fix;
pub mod pollen_count_gauge;
pub mod rain_intensity;
pub mod temperature_forecast;
pub mod wind_speed;

use crate::{
    config::Config,
    data::source::weather::{self, Birch, Grass, Location, Pollen, Rain, Temperature, Wind},
    message_subscriptions::polling,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        source_reading::source_reading,
        tooltip_body::Tooltip,
    },
};

#[derive(Default)]
pub struct WeatherGroup {
    temp: LastPoll<Temperature>,
    wind: LastPoll<Wind>,
    rain: LastPoll<Rain>,
    birch: LastPoll<Pollen<Birch>>,
    grass: LastPoll<Pollen<Grass>>,
    location: LastPoll<Location>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum WeatherMessage {
    Temp(Result<Temperature, weather::Error>),
    Wind(Result<Wind, weather::Error>),
    Rain(Result<Rain, weather::Error>),
    Birch(Result<Pollen<Birch>, weather::Error>),
    Grass(Result<Pollen<Grass>, weather::Error>),
    Location(Result<Location, weather::Error>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
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
            polling::<Pollen<Birch>>(here).map(WeatherMessage::Birch),
            polling::<Pollen<Grass>>(here).map(WeatherMessage::Grass),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temp(reading) => self.temp.receive(reading),
            WeatherMessage::Wind(reading) => self.wind.receive(reading),
            WeatherMessage::Rain(reading) => self.rain.receive(reading),
            WeatherMessage::Birch(reading) => self.birch.receive(reading),
            WeatherMessage::Grass(reading) => self.grass.receive(reading),
            WeatherMessage::Location(reading) => self.location.receive(reading),
            WeatherMessage::Menu(m) => {
                self.menu.update(m);
            }
            WeatherMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::WeatherTemp => self.temp.tooltip(),
            HoverTarget::WeatherWind => self.wind.tooltip(),
            HoverTarget::WeatherRain => self.rain.tooltip(),
            HoverTarget::PollenBirch => self.birch.tooltip(),
            HoverTarget::PollenGrass => self.grass.tooltip(),
            HoverTarget::Location => self.location.live_tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, WeatherMessage>> {
        self.location.get()?;
        source_reading::<WeatherMessage>(config, self.temp.display(), 1.0)?;

        let face = move || {
            let reading = source_reading(config, self.temp.display(), 1.0)
                .unwrap_or_else(|| Row::new().into());
            hoverable(reading, HoverTarget::WeatherTemp, WeatherMessage::Tooltip)
        };

        let content = move |reveal: f32| {
            ReadingRow::new()
                .opacity(reveal)
                .push(Reading::new(self.wind.display(), HoverTarget::WeatherWind))
                .push(Reading::new(self.rain.present(), HoverTarget::WeatherRain))
                .push(Reading::new(self.birch.present(), HoverTarget::PollenBirch))
                .push(Reading::new(self.grass.present(), HoverTarget::PollenGrass))
                .push(Reading::new(self.location.display(), HoverTarget::Location))
                .row(config, WeatherMessage::Tooltip)
                .unwrap_or_else(Row::new)
                .into()
        };

        Some(
            self.menu
                .slot(face, content, WeatherMessage::Menu)
                .view(config, position),
        )
    }
}
