//! The `font` config group. The family name is resolved once at startup via
//! `rust-fontconfig` (querying the system default `sans-serif`); `size_px` is
//! a plain TOML field. Both are stored in module-scope `OnceLock`s so their
//! storage lives for the program lifetime, and `name()` returns `&'static str`
//! without `Box::leak`.

use std::sync::OnceLock;

use iced::Font;
use serde::Deserialize;

/// User-overridable font config from `[font]` in `odin.toml`.
/// `name = None` means "use whatever fontconfig resolves `sans-serif` to".
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontSpec {
    pub name: Option<String>,
    pub size_px: f32,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            name: None,
            size_px: 12.0,
        }
    }
}

/// Resolved family name. Owned by the static so its storage is program-
/// lifetime — `as_str()` returns `&'static str` with no `Box::leak`.
static FONT_NAME: OnceLock<String> = OnceLock::new();

/// Resolved font size in logical pixels.
static FONT_SIZE: OnceLock<f32> = OnceLock::new();

/// Populate the statics from the user's config + fontconfig. Called once at
/// startup from `Config::load()`. Idempotent — subsequent calls are no-ops.
pub fn init(name_override: Option<&str>, size_px: f32) {
    let resolved = name_override
        .map(str::to_owned)
        .or_else(query_fontconfig_family)
        .unwrap_or_else(|| "sans-serif".to_owned());
    let _ = FONT_NAME.set(resolved);
    let _ = FONT_SIZE.set(size_px);
}

/// The resolved family name as `&'static str` — safe to pass to
/// `iced::Font::with_name`. Requires `init()` to have been called.
pub fn name() -> &'static str {
    FONT_NAME.get().expect("font::init not called").as_str()
}

/// The resolved font size in logical pixels.
pub fn size() -> f32 {
    *FONT_SIZE.get().expect("font::init not called")
}

/// The resolved `iced::Font`.
pub fn iced_font() -> Font {
    Font::with_name(name())
}

/// Query `rust-fontconfig` for the system default sans-serif family.
/// Returns `None` on any failure (no fonts installed, parse error, …).
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
