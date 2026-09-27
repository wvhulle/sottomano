use std::fmt::{self, Display, Formatter};

use iced::Color;

use crate::{
    config::palette::Palette,
    widgets::{
        font_awesome::{Brands, FontAwesome, Regular, Solid},
        simple_icons::SimpleIcon,
        tooltip_body::TooltipBody,
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
    FontAwesome(FontAwesome),
    SimpleIcon(SimpleIcon),
}

impl From<FontAwesome> for Glyph {
    fn from(icon: FontAwesome) -> Self {
        Glyph::FontAwesome(icon)
    }
}

impl From<Solid> for Glyph {
    fn from(icon: Solid) -> Self {
        Glyph::FontAwesome(FontAwesome::Solid(icon))
    }
}

impl From<Regular> for Glyph {
    fn from(icon: Regular) -> Self {
        Glyph::FontAwesome(FontAwesome::Regular(icon))
    }
}

impl From<Brands> for Glyph {
    fn from(icon: Brands) -> Self {
        Glyph::FontAwesome(FontAwesome::Brands(icon))
    }
}

impl From<SimpleIcon> for Glyph {
    fn from(icon: SimpleIcon) -> Self {
        Glyph::SimpleIcon(icon)
    }
}

pub trait BarWidget {
    fn icon(&self) -> Glyph;

    fn fallback_icon() -> Glyph
    where
        Self: Sized,
    {
        Solid::CircleQuestion.into()
    }

    fn label(&self) -> Option<String> {
        None
    }

    fn level(&self) -> Level {
        Level::Ok
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

    fn disabled(&self) -> bool {
        false
    }

    fn failed(&self) -> bool {
        false
    }

    fn tooltip(&self) -> TooltipBody;
}

pub trait Occasional: BarWidget {
    fn present(&self) -> bool;
}
