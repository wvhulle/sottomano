use iced::{Color, Element, Theme};

use crate::widgets::{
    phosphor::{Phosphor, Weight, view_phosphor},
    simple_icons::{SimpleIcon, view_simple_icon},
    weather_icons::WeatherIcon,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconGlyph {
    Phosphor(Phosphor),
    SimpleIcon(SimpleIcon),
    WeatherIcon(WeatherIcon),
}

impl IconGlyph {
    #[must_use]
    pub const fn with_weight(self, weight: Weight) -> Self {
        match self {
            IconGlyph::Phosphor(icon) => IconGlyph::Phosphor(icon.with_weight(weight)),
            IconGlyph::SimpleIcon(_) | IconGlyph::WeatherIcon(_) => self,
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(
        self,
        size: f32,
        color: impl Fn(&Theme) -> Color + 'a,
    ) -> Element<'a, Message> {
        match self {
            IconGlyph::Phosphor(icon) => view_phosphor(icon, size, color),
            IconGlyph::SimpleIcon(icon) => view_simple_icon(icon, size, color),
            IconGlyph::WeatherIcon(icon) => icon.view(size, color),
        }
    }
}
