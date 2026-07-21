//! Font configuration.

use std::sync::OnceLock;

use iced::Font;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontSpec {
    pub name: Option<String>,
    pub size_px: f32,
    pub icon_size: f32,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            name: None,
            size_px: 12.0,
            icon_size: 14.0,
        }
    }
}

static FONT_NAME: OnceLock<String> = OnceLock::new();
static FONT_SIZE: OnceLock<f32> = OnceLock::new();

pub fn init(name_override: Option<&str>, size_px: f32) {
    let resolved = name_override
        .map(str::to_owned)
        .or_else(query_fontconfig_family)
        .unwrap_or_else(|| "sans-serif".to_owned());
    let _ = FONT_NAME.set(resolved);
    let _ = FONT_SIZE.set(size_px);
}

pub fn name() -> &'static str {
    FONT_NAME.get().expect("font::init not called").as_str()
}

pub fn size() -> f32 {
    *FONT_SIZE.get().expect("font::init not called")
}

pub fn iced_font() -> Font {
    Font::with_name(name())
}

fn query_fontconfig_family() -> Option<String> {
    use rust_fontconfig::{FcFontCache, FcPattern};

    let cache = FcFontCache::build();
    let pattern = FcPattern {
        family: Some("sans-serif".to_owned()),
        ..Default::default()
    };
    let matched = cache.query(&pattern, &mut Vec::new())?;
    let meta = cache.get_metadata_by_id(&matched.id)?;
    meta.name.clone().or_else(|| meta.family.clone())
}
