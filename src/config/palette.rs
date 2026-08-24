use iced::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct Palette {
    #[serde(with = "css_color")]
    pub bar_background: Color,
    #[serde(with = "css_color")]
    pub bar_foreground: Color,
    #[serde(with = "css_color")]
    pub inactive: Color,
    #[serde(with = "css_color")]
    pub urgent: Color,
    #[serde(with = "css_color")]
    pub warning: Color,
    #[serde(with = "css_color")]
    pub focus: Color,
    #[serde(with = "css_color")]
    pub dim: Color,
}

impl Palette {
    pub fn popup_background(&self) -> Color {
        Color {
            a: 1.0,
            ..self.bar_background
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            bar_background: css("transparent"),
            bar_foreground: css("#ffffff"),
            inactive: css("#bdae93"),
            urgent: css("#fb4934"),
            warning: css("#fabd2f"),
            focus: css("#8ec07c"),
            dim: css("#FFFFFF80"),
        }
    }
}

fn css(s: &str) -> Color {
    to_iced(color::parse_color(s).expect("invalid default color"))
}

fn to_iced(dynamic: color::DynamicColor) -> Color {
    let [r, g, b, a] = dynamic
        .convert(color::ColorSpaceTag::Srgb)
        .to_alpha_color::<color::Srgb>()
        .components;
    Color { r, g, b, a }
}

mod css_color {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, de::Error as _};

    use super::{Color, to_iced};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        let s = String::deserialize(d)?;
        color::DynamicColor::from_str(&s)
            .map(to_iced)
            .map_err(D::Error::custom)
    }
}
