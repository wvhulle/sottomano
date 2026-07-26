//! odin's `Weather`: a temperature handle that unfolds a location pin, current
//! rain, and birch/grass pollen gauges. Hidden until situo has a fix. odin's
//! hover forecast-chart popup is not ported; hovering shows the tooltips.

use iced::{
    Alignment, Element, Subscription,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget, Tooltip,
        weather::{Location, Pollen, Rain, Temperature},
    },
    layer_surface::Position,
    tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{
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
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, WeatherMessage>> {
        // Temperature is the persistent handle; without it the group hides.
        let temp = self.temp.get()?;
        let handle_reading = source_reading(config, Some(temp as &dyn BarWidget), 1.0)?;
        let handle = mouse_area(hoverable(
            handle_reading,
            HoverTarget::WeatherTemp,
            WeatherMessage::Tooltip,
        ))
        .on_press(WeatherMessage::Menu(MenuMessage::Toggle))
        .into();

        let reveal = self.menu.reveal();
        let pills = [
            self.location
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::Location)),
            self.rain
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::WeatherRain)),
            self.birch
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::PollenBirch)),
            self.grass
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::PollenGrass)),
        ]
        .into_iter()
        .flatten()
        .filter_map(|(widget, target)| {
            let reading = source_reading(config, Some(widget), reveal)?;
            Some(hoverable(reading, target, WeatherMessage::Tooltip))
        });

        let content = Row::with_children(pills)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);

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
