use color::{AlphaColor, Srgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct Palette {
    #[serde(with = "css_color")]
    pub background: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub text: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub primary: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub success: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub warning: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub danger: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub background_weak: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub background_strong: AlphaColor<Srgb>,
    #[serde(with = "css_color")]
    pub secondary: AlphaColor<Srgb>,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: css("#0b0e14"),
            text: css("#e6e1cf"),
            primary: css("#95e6cb"),
            success: css("#aad94c"),
            warning: css("#ffb454"),
            danger: css("#f07178"),
            background_weak: css("#202229"),
            background_strong: css("#3e4b59"),
            secondary: css("#bfbdb6"),
        }
    }
}

impl Palette {
    #[must_use]
    pub fn light() -> Self {
        Self {
            background: css("#fcfcfc"),
            text: css("#5c6166"),
            primary: css("#4cbf99"),
            success: css("#86b300"),
            warning: css("#f2ae49"),
            danger: css("#e65050"),
            background_weak: css("#eceff1"),
            background_strong: css("#d3d7db"),
            secondary: css("#8a9199"),
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
