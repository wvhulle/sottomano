use iced::{Color, Theme, theme};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct Palette {
    #[serde(with = "css_color")]
    pub surface: Color,
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
    pub success: Color,
    #[serde(with = "css_color")]
    pub info: Color,
    #[serde(with = "css_color")]
    pub dim: Color,
    #[serde(with = "css_color")]
    pub surface_raised: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            surface: css("#0b0e14"),
            bar_foreground: css("#e6e1cf"),
            inactive: css("#bfbdb6"),
            urgent: css("#f07178"),
            warning: css("#ffb454"),
            focus: css("#95e6cb"),
            success: css("#aad94c"),
            info: css("#59c2ff"),
            dim: css("#3e4b59"),
            surface_raised: css("#202229"),
        }
    }
}

impl From<&Palette> for Theme {
    fn from(palette: &Palette) -> Self {
        Theme::custom(
            String::from("umbra"),
            theme::Palette {
                background: palette.surface,
                text: palette.bar_foreground,
                primary: palette.focus,
                success: palette.success,
                warning: palette.warning,
                danger: palette.urgent,
            },
        )
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
