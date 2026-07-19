//! The `palette` config group (matches odin's `ColorPalette.qml`).

use iced::Color;
use serde::Deserialize;

/// A `#rrggbb` / `#rrggbbaa` color, as odin writes them in config.json.
#[derive(Debug, Clone, Copy)]
pub struct Rgb(pub Color);

impl Rgb {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Color::from_rgb(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
        ))
    }

    /// This color with its alpha replaced — mirrors
    /// `ConfigLoader.withBgOpacity`.
    pub fn with_alpha(self, alpha: f32) -> Color {
        Color { a: alpha, ..self.0 }
    }

    fn parse(hex: &str) -> Option<Color> {
        let digits = hex.strip_prefix('#')?;
        let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
        match digits.len() {
            6 => Some(Color::from_rgb8(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Color::from_rgba8(
                byte(0)?,
                byte(2)?,
                byte(4)?,
                byte(6)? as f32 / 255.0,
            )),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(d)?;
        let color = Rgb::parse(&hex)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid color {hex:?}")))?;
        Ok(Rgb(color))
    }
}

/// The `palette` group. Defaults are odin's gruvbox-dark base16 (see
/// `ColorPalette.qml`).
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Palette {
    pub bg: Rgb,
    pub surface: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub critical_tint: Rgb,
    pub time_tint: Rgb,
    pub brightness_tint: Rgb,
    pub network_tint: Rgb,
    pub power_tint: Rgb,
    pub system_tint: Rgb,
    pub audio_tint: Rgb,
    pub gradient: Rgb,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            bg: Rgb::rgb(0x1d, 0x20, 0x21),
            surface: Rgb::rgb(0x50, 0x49, 0x45),
            text: Rgb::rgb(0xd5, 0xc4, 0xa1),
            muted: Rgb::rgb(0xbd, 0xae, 0x93),
            critical_tint: Rgb::rgb(0xfb, 0x49, 0x34),
            time_tint: Rgb::rgb(0xfe, 0x80, 0x19),
            brightness_tint: Rgb::rgb(0xfa, 0xbd, 0x2f),
            network_tint: Rgb::rgb(0xb8, 0xbb, 0x26),
            power_tint: Rgb::rgb(0x8e, 0xc0, 0x7c),
            system_tint: Rgb::rgb(0x83, 0xa5, 0x98),
            audio_tint: Rgb::rgb(0xd3, 0x86, 0x9b),
            gradient: Rgb::rgb(0x0b, 0x0e, 0x0f),
        }
    }
}
