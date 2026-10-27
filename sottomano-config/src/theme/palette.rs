use color::{AlphaColor, Srgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct Palette {
    #[serde(with = "css_color")]
    pub surface: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub bar_foreground: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub inactive: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub urgent: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub warning: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub focus: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub success: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub info: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub dim: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub surface_raised: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub frame_outline: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub secondary_foreground: AlphaColor<Srgb>,
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
            frame_outline: css("#3e4b59"),
            secondary_foreground: css("#bfbdb6"),
        }
    }
}

fn css(s: &str) -> AlphaColor<Srgb> {
    in_srgb(color::parse_color(s).expect("invalid default color"))
}

fn in_srgb(dynamic: color::DynamicColor) -> AlphaColor<Srgb> {
    dynamic
        .convert(color::ColorSpaceTag::Srgb)
        .to_alpha_color::<Srgb>()
}

mod css_color {
    use std::str::FromStr;

    use color::{AlphaColor, Srgb};
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    use super::in_srgb;

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(srgb: &AlphaColor<Srgb>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&format_args!("{:x}", srgb.to_rgba8()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<AlphaColor<Srgb>, D::Error> {
        let s = String::deserialize(d)?;
        color::DynamicColor::from_str(&s)
            .map(in_srgb)
            .map_err(D::Error::custom)
    }
}
