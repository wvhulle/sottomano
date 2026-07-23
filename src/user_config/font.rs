//! Font configuration.

use std::{collections::HashSet, sync::Arc};

use iced::Font;
use serde::Deserialize;

/// The `[font]` config table: what the user asked for. Resolved into a
/// [`BarFont`] at load (see [`BarFont::resolve`]).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub name: Option<String>,
    pub text_size_px: f32,
    pub icon_size_px: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            name: None,
            text_size_px: 14.0,
            icon_size_px: 15.0,
        }
    }
}

/// The bar's resolved font: a concrete family plus everything derived from its
/// file, all owned — no borrowed `Face`, no globals. Built once at startup by
/// [`resolve`](Self::resolve) and carried in `RuntimeConfig`. The heavy fields
/// are `Arc`-shared so cloning the config per output is cheap.
#[derive(Debug, Clone)]
pub struct RuntimeFont {
    /// Prebuilt iced font. Its family name is the one value that must be
    /// `&'static` (iced's [`Font::with_name`] takes no owned name), so it is
    /// leaked once at [`resolve`](Self::resolve) — the sole leak.
    font: Font,
    text_size_px: f32,
    pub icon_size_px: f32,
    /// The font-file bytes, owned, for registering the family with iced.
    data: Arc<[u8]>,
    /// Descent in px at the bar text size, precomputed from the face.
    descent_px: f32,
    /// The codepoints the family has glyphs for, precomputed from its cmap.
    covered: Arc<HashSet<char>>,
}

impl RuntimeFont {
    /// Resolve `config`'s family, then load and parse its font file, keeping
    /// the bytes and the metrics/coverage derived from it. Panics if no
    /// sans-serif font can be loaded at all: a bar with no text font is
    /// unusable, so it fails loudly rather than rendering tofu. Call once,
    /// at startup.
    pub fn resolve(config: &FontConfig) -> Self {
        let name = config
            .name
            .clone()
            .or_else(|| fc_match("sans-serif", "%{family}"))
            .unwrap_or_else(|| "sans-serif".to_owned());
        // Read the resolved family's actual file so it can be registered with
        // iced (see `data`). iced's cosmic-text DB only resolves
        // `Font::with_name` against faces it was explicitly handed; without the
        // real file, bar text falls back to the Font Awesome icon font (tofu).
        let bytes = fc_match(&name, "%{file}")
            .and_then(|path| std::fs::read(path).ok())
            .expect("no readable sans-serif font found");
        let face = ttf_parser::Face::parse(&bytes, 0).expect("sans-serif font failed to parse");
        let descent_px = f32::from(face.descender().unsigned_abs())
            / f32::from(face.units_per_em())
            * config.text_size_px;
        let covered = covered_chars(&face);
        // The one unavoidable leak: iced's `Font::with_name` requires `&'static`.
        let font = Font::with_name(String::leak(name));
        Self {
            font,
            text_size_px: config.text_size_px,
            icon_size_px: config.icon_size_px,
            data: Arc::from(bytes),
            descent_px,
            covered: Arc::new(covered),
        }
    }

    pub fn size(&self) -> f32 {
        self.text_size_px
    }

    pub fn iced_font(&self) -> Font {
        self.font
    }

    /// The resolved family's font-file bytes, for registering with iced so
    /// [`iced_font`](Self::iced_font) resolves instead of falling back to the
    /// icon font.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Whether the bar font has a glyph for `ch`.
    pub fn covers(&self, ch: char) -> bool {
        self.covered.contains(&ch)
    }

    /// The font's descent (space below the baseline) in px at the bar text
    /// size. iced lays text out in its full line box — ascent + descent +
    /// leading — and centres that box, so a label with no descenders (e.g.
    /// "odin") sits too high by exactly this much; callers centring a label
    /// between edge indicators pad its top by this.
    pub fn descent_px(&self) -> f32 {
        self.descent_px
    }
}

/// Every Unicode codepoint the face has a glyph for, from its cmap subtables.
fn covered_chars(face: &ttf_parser::Face) -> HashSet<char> {
    let mut set = HashSet::new();
    if let Some(cmap) = face.tables().cmap {
        for subtable in cmap.subtables {
            if subtable.is_unicode() {
                subtable.codepoints(|cp| {
                    if let Some(ch) = char::from_u32(cp) {
                        set.insert(ch);
                    }
                });
            }
        }
    }
    set
}

/// The family fontconfig would pick for a single codepoint, as an iced
/// [`Font`]. The family's file is already in iced's font DB (loaded from the
/// system), so [`Font::with_name`] resolves it and cosmic-text renders the
/// glyph from it without consulting its per-script fallback table — matching
/// how Qt/GTK (and thus odin-qml) render glyphs the bar font lacks, e.g. the
/// terminal-title progress marker `⠐` (U+2810). Spawns `fc-match`; call from a
/// blocking task and cache the result, never from the render path.
pub fn fallback_font(ch: char) -> Option<Font> {
    fc_match(&format!(":charset={:x}", ch as u32), "%{family}")
        .map(|family| Font::with_name(String::leak(family)))
}

/// Query one field (`format`, e.g. `%{family}` or `%{file}`) of fontconfig's
/// best match for `pattern` via `fc-match`, which reads libfontconfig's
/// prebuilt on-disk cache and returns in milliseconds. This deliberately avoids
/// parsing every font file in-process (as a pure-Rust fontconfig cache build
/// would), which stalled startup by seconds on every launch.
fn fc_match(pattern: &str, format: &str) -> Option<String> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", format, pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    // fontconfig may emit a comma-separated list (e.g. family aliases); take the
    // first.
    let value = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
