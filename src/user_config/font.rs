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
            icon_size_px: 14.0,
        }
    }
}

static FONT_NAME: OnceLock<String> = OnceLock::new();
static FONT_SIZE: OnceLock<f32> = OnceLock::new();
static FONT_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();

pub fn init(name_override: Option<&str>, size_px: f32) {
    let resolved = name_override
        .map(str::to_owned)
        .or_else(|| fc_match("sans-serif", "%{family}"))
        .unwrap_or_else(|| "sans-serif".to_owned());
    // Read the resolved family's actual font file so it can be registered with
    // iced at startup (see `sans_serif_data`). iced's cosmic-text DB only resolves
    // `Font::with_name` against faces it was explicitly handed, and its generic
    // sans-serif fallback points at "Open Sans" (not installed here); without the
    // real file, bar text falls back to whatever else is loaded — the Font Awesome
    // icon font — and renders as tofu.
    let data = fc_match(&resolved, "%{file}").and_then(|path| std::fs::read(path).ok());
    let _ = FONT_NAME.set(resolved);
    let _ = FONT_SIZE.set(size_px);
    let _ = FONT_DATA.set(data);
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

/// Bytes of the resolved sans-serif font file, for registering with iced at
/// startup so [`iced_font`] actually resolves instead of falling back to the icon
/// font. `None` if the file could not be resolved or read.
pub fn sans_serif_data() -> Option<&'static [u8]> {
    FONT_DATA.get().and_then(|d| d.as_deref())
}

/// The parsed bar font face, or `None` if the font file could not be read or
/// parsed. Parsed once and cached.
fn face() -> Option<&'static ttf_parser::Face<'static>> {
    static FACE: OnceLock<Option<ttf_parser::Face<'static>>> = OnceLock::new();
    FACE.get_or_init(|| sans_serif_data().and_then(|d| ttf_parser::Face::parse(d, 0).ok()))
        .as_ref()
}

/// Whether the resolved bar font has a glyph for `ch`. Assumes covered if the
/// font file could not be read or parsed, degrading to cosmic-text's own fallback.
pub fn bar_font_covers(ch: char) -> bool {
    face().is_none_or(|face| face.glyph_index(ch).is_some())
}

/// The font's descent (space below the baseline) in px at the bar text size.
/// iced lays text out in its full line box — ascent + descent + leading — and
/// centres that box, so a label with no descenders (e.g. "odin") sits too high
/// by exactly this much. Callers that centre a label between edge indicators pad
/// its top by this to drop the glyphs back onto the centre line (see
/// `bar_layout::workspaces_group`). Derived from the font's real metrics, so it
/// tracks the font and size; `0` when the font can't be read.
pub fn descent_px() -> f32 {
    static DESCENT: OnceLock<f32> = OnceLock::new();
    *DESCENT.get_or_init(|| {
        face().map_or(0.0, |face| {
            f32::from(face.descender().unsigned_abs()) / f32::from(face.units_per_em()) * size()
        })
    })
}

/// The family fontconfig would pick for a single codepoint, as an iced [`Font`].
/// The family's file is already in iced's font DB (loaded from the system), so
/// [`Font::with_name`] resolves it and cosmic-text renders the glyph from it
/// without consulting its per-script fallback table — matching how Qt/GTK (and
/// thus odin-qml) render glyphs the bar font lacks, e.g. the terminal-title
/// progress marker `⠐` (U+2810). Spawns `fc-match`; call from a blocking task
/// and cache the result, never from the render path.
pub fn fallback_font(ch: char) -> Option<Font> {
    fc_match(&format!(":charset={:x}", ch as u32), "%{family}")
        .map(|family| Font::with_name(String::leak(family)))
}

/// Query one field (`format`, e.g. `%{family}` or `%{file}`) of fontconfig's best
/// match for `pattern` via `fc-match`, which reads libfontconfig's prebuilt
/// on-disk cache and returns in milliseconds. This deliberately avoids parsing
/// every font file in-process (as a pure-Rust fontconfig cache build would),
/// which stalled startup by seconds on every launch.
fn fc_match(pattern: &str, format: &str) -> Option<String> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", format, pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    // fontconfig may emit a comma-separated list (e.g. family aliases); take the first.
    let value = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
