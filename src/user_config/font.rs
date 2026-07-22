//! Font configuration.

use std::sync::OnceLock;

use iced::Font;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontSpec {
    pub name: Option<String>,
    pub text_size_px: f32,
    pub icon_size_px: f32,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            name: None,
            text_size_px: 12.0,
            icon_size_px: 12.0,
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

/// Resolve the system `sans-serif` alias to a concrete family name via
/// `fc-match`, which reads libfontconfig's prebuilt on-disk cache and returns
/// in milliseconds. This deliberately avoids parsing every font file in-process
/// (as a pure-Rust fontconfig cache build would), which stalled startup by
/// seconds on every launch.
fn query_fontconfig_family() -> Option<String> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{family}\n", "sans-serif"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    // fontconfig may emit a comma-separated family alias list; take the first.
    let family = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!family.is_empty()).then_some(family)
}
