use iced::{
    Element, Subscription, Task,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::source::{
        Tooltip,
        weather::{self, Birch, Grass, Location, Pollen, Rain, Temperature},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::{pills::Pills, source_cell::SourceCell},
    widgets::{
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        source_reading::source_reading,
    },
};

#[derive(Default)]
pub struct WeatherGroup {
    temp: SourceCell<Temperature>,
    rain: SourceCell<Rain>,
    birch: SourceCell<Pollen<Birch>>,
    grass: SourceCell<Pollen<Grass>>,
    location: SourceCell<Location>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum WeatherMessage {
    Temp(Result<Option<Temperature>, weather::Error>),
    Rain(Result<Option<Rain>, weather::Error>),
    Birch(Result<Option<Pollen<Birch>>, weather::Error>),
    Grass(Result<Option<Pollen<Grass>>, weather::Error>),
    Location(Result<Option<Location>, weather::Error>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl WeatherGroup {
    pub fn subscription() -> Subscription<WeatherMessage> {
        Subscription::batch([
            SourceCell::subscription().map(WeatherMessage::Temp),
            SourceCell::subscription().map(WeatherMessage::Rain),
            SourceCell::subscription().map(WeatherMessage::Birch),
            SourceCell::subscription().map(WeatherMessage::Grass),
            SourceCell::subscription().map(WeatherMessage::Location),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage) -> Task<WeatherMessage> {
        match msg {
            WeatherMessage::Temp(reading) => self.temp.receive(reading),
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
            HoverTarget::WeatherRain => self.rain.tooltip(),
            HoverTarget::PollenBirch => self.birch.tooltip(),
            HoverTarget::PollenGrass => self.grass.tooltip(),
            HoverTarget::Location => self.location.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, WeatherMessage>> {
        let handle_reading = source_reading(config, self.temp.display(), 1.0)?;
        let handle = button(hoverable(
            handle_reading,
            HoverTarget::WeatherTemp,
            WeatherMessage::Tooltip,
        ))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(WeatherMessage::Menu(MenuMessage::Toggle))
        .into();

        let reveal = self.menu.reveal();
        let content = Pills::new()
            .alpha(reveal)
            .push(self.location.display(), HoverTarget::Location)
            .push(self.rain.display(), HoverTarget::WeatherRain)
            .push(self.birch.display(), HoverTarget::PollenBirch)
            .push(self.grass.display(), HoverTarget::PollenGrass)
            .row(config, WeatherMessage::Tooltip)
            .unwrap_or_else(Row::new);

        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            WeatherMessage::Menu,
        ))
    }
}
