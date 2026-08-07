use iced::{
    Element, Subscription,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::source::{
        BarWidget, Tooltip,
        weather::{Location, Pollen, Rain, Temperature},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::{pills::Pills, source_cell::SourceCell},
    widgets::{
        flat_button,
        icon_gauge::source_reading,
        menu_group::{MenuGroup, MenuMessage},
    },
};

pub struct WeatherGroup {
    temp: SourceCell<Temperature>,
    rain: SourceCell<Rain>,
    birch: SourceCell<Pollen<false>>,
    grass: SourceCell<Pollen<true>>,
    location: SourceCell<Location>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum WeatherMessage {
    Temp(Option<Temperature>),
    Rain(Option<Rain>),
    Birch(Option<Pollen<false>>),
    Grass(Option<Pollen<true>>),
    Location(Option<Location>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for WeatherMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
}

impl WeatherGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            temp: SourceCell::default(),
            rain: SourceCell::default(),
            birch: SourceCell::default(),
            grass: SourceCell::default(),
            location: SourceCell::default(),
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<WeatherMessage> {
        Subscription::batch([
            SourceCell::subscription().map(WeatherMessage::Temp),
            SourceCell::subscription().map(WeatherMessage::Rain),
            SourceCell::subscription().map(WeatherMessage::Birch),
            SourceCell::subscription().map(WeatherMessage::Grass),
            SourceCell::subscription().map(WeatherMessage::Location),
        ])
    }

    pub fn update(&mut self, msg: WeatherMessage) {
        match msg {
            WeatherMessage::Temp(reading) => self.temp.load(reading),
            WeatherMessage::Rain(reading) => self.rain.load(reading),
            WeatherMessage::Birch(reading) => self.birch.load(reading),
            WeatherMessage::Grass(reading) => self.grass.load(reading),
            WeatherMessage::Location(reading) => self.location.load(reading),
            WeatherMessage::Menu(m) => {
                self.menu.update(m);
            }
            WeatherMessage::Tooltip(_) => {}
        }
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
        let temp = self.temp.get()?;
        let handle_reading = source_reading(config, Some(temp as &dyn BarWidget), 1.0)?;
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
            .push(self.location.get(), HoverTarget::Location)
            .push(self.rain.get(), HoverTarget::WeatherRain)
            .push(self.birch.get(), HoverTarget::PollenBirch)
            .push(self.grass.get(), HoverTarget::PollenGrass)
            .row(config)
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
