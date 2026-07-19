//! The `font` config group (matches odin's `FontConfig.qml`).
//!
//! `sans`/`mono` strings are fontconfig generic aliases; Qt resolves them to
//! concrete system fonts at runtime. iced 0.14's [`iced::Font::with_name`] takes
//! `&'static str`, so any non-static alias forces a one-time [`Box::leak`] at
//! config-load. Resolving here (rather than per render call) bounds the leak to
//! the configured family names — see `font_family`. TODO: resolve the alias via
//! fontconfig at startup instead of hardcoding the current machine's default.

use iced::Font;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Spec {
    pub family: String,
    pub mono: String,
    pub size: u16,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_string(),
            mono: "monospace".to_string(),
            size: 12,
        }
    }
}

impl Spec {
    /// Resolve fontconfig alias strings to concrete [`Font`] values.
    pub fn resolve(self) -> FontConfig {
        FontConfig {
            sans: resolve_family(&self.family),
            #[allow(dead_code)]
            mono: resolve_family(&self.mono),
            size: self.size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FontConfig {
    pub sans: Font,
    #[allow(dead_code)]
    pub mono: Font,
    pub size: u16,
}

impl Default for FontConfig {
    fn default() -> Self {
        Spec::default().resolve()
    }
}

fn resolve_family(name: &str) -> Font {
    match name {
        "sans-serif" | "" => Font::with_name("IBM Plex Sans"),
        "monospace" => Font::with_name("Lilex Nerd Font Mono"),
        name => Font::with_name(Box::leak(name.to_string().into_boxed_str())),
    }
}