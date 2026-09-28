use std::fmt::{self, Display, Formatter};

use iced::{Color, Element};

use crate::{
    config::palette::Palette,
    widgets::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        svg_icon,
        tooltip_body::TooltipBody,
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    #[default]
    Ok,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Watt,
}

impl Display for Unit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Unit::Percent => "%",
            Unit::Watt => "W",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Gauge {
        percent: i64,
        unit: Unit,
    },
    #[allow(dead_code)]
    Slider {
        percent: i64,
        unit: Unit,
    },
}

impl Value {
    pub fn percent(self) -> i64 {
        match self {
            Value::Gauge { percent, .. } | Value::Slider { percent, .. } => percent,
        }
    }
}

pub fn level_color(palette: &Palette, level: Level) -> Color {
    match level {
        Level::Ok => palette.bar_foreground,
        Level::Warning => palette.warning,
        Level::Critical => palette.urgent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Phosphor(Phosphor),
    SimpleIcon(SimpleIcon),
    WeatherIcon(WeatherIcon),
}

impl Glyph {
    #[must_use]
    pub const fn with_weight(self, weight: Weight) -> Self {
        match self {
            Glyph::Phosphor(icon) => Glyph::Phosphor(icon.with_weight(weight)),
            Glyph::SimpleIcon(_) | Glyph::WeatherIcon(_) => self,
        }
    }

    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        match self {
            Glyph::Phosphor(icon) => svg_icon::view(icon.svg_data(), size, color),
            Glyph::SimpleIcon(icon) => icon.view(size, color),
            Glyph::WeatherIcon(icon) => icon.view(size, color),
        }
    }
}

pub trait BarWidget {
    fn icon(&self) -> Glyph;

    fn fallback_icon() -> Glyph
    where
        Self: Sized,
    {
        Phosphor::Question(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        None
    }

    fn state(&self) -> WidgetState {
        WidgetState::Idle
    }

    fn value(&self) -> Option<Value> {
        None
    }

    fn badge(&self) -> Option<u32> {
        None
    }

    fn live_fraction(&self) -> f32 {
        1.0
    }

    fn tooltip(&self) -> TooltipBody;
}

pub trait Occasional: BarWidget {
    fn present(&self) -> bool;
}
