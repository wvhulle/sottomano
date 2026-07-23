//! The resolved config, built once at startup from [`super::disk`]: the font is
//! loaded and parsed, and colors are converted to `iced::Color`. Every field is
//! plain data — the resolved value *is* the answer, so there are no accessor
//! methods; callers read the fields directly.

use std::{collections::HashSet, sync::Arc};

use color::{ColorSpaceTag, DynamicColor, Srgb};
use iced::{Color, Font};

use super::disk::{self, DiskConfig, DiskFontConfig, DiskPaletteConfig, Layout};

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub palette: ResolvedPaletteConfig,
    pub layout: Layout,
    pub font: ResolvedFontConfig,
    /// Corner radius actually drawn: the configured radius when floating, else
    /// `0`. Precomputed so views read a field instead of recomputing.
    pub chrome_radius: f32,
}

#[derive(Debug, Clone)]
pub struct ResolvedPaletteConfig {
    pub bar_background: Color,
    pub bar_foreground: Color,
    pub inactive: Color,
    pub urgent: Color,
    pub warning: Color,
    pub focus: Color,
    pub dim: Color,
}

/// The bar's resolved font: a concrete family plus everything derived from its
/// file, all owned — no borrowed `Face`, no globals. The heavy fields are
/// `Arc`-shared so cloning the config per output is cheap.
#[derive(Debug, Clone)]
pub struct ResolvedFontConfig {
    /// Prebuilt iced font. Its family name is the one value that must be
    /// `&'static` (iced's [`Font::with_name`] takes no owned name), so it is
    /// leaked once here — the sole leak.
    pub iced_font: Font,
    pub text_size_px: f32,
    pub icon_size_px: f32,
    /// The font-file bytes, owned, for registering the family with iced.
    pub data: Arc<[u8]>,
    /// Descent in px at the bar text size, precomputed from the face. iced lays
    /// text out in its full line box (ascent + descent + leading) and centres
    /// that box, so a label with no descenders sits too high by exactly this
    /// much; callers centring a label between edge indicators pad its top by it.
    pub descent_px: f32,
    /// The codepoints the family has glyphs for, precomputed from its cmap.
    pub covered: Arc<HashSet<char>>,
}

/// Read the config from disk and resolve it — load the font and convert colors.
/// Panics if no sans-serif font can be loaded at all: a bar with no text font is
/// unusable, so it fails loudly rather than rendering tofu. Call once, at startup.
pub fn load() -> ResolvedConfig {
    let path = disk::config_path();
    let user = match disk::read(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {} {e}; using defaults", path.display());
            DiskConfig::default()
        }
    };
    let chrome_radius = if user.layout.floating {
        f32::from(user.layout.corner_radius_px)
    } else {
        0.0
    };
    ResolvedConfig {
        palette: resolve_palette(&user.palette),
        layout: user.layout,
        font: resolve_font(&user.font),
        chrome_radius,
    }
}

fn resolve_palette(palette: &DiskPaletteConfig) -> ResolvedPaletteConfig {
    ResolvedPaletteConfig {
        bar_background: to_iced(palette.bar_background),
        bar_foreground: to_iced(palette.bar_foreground),
        inactive: to_iced(palette.inactive),
        urgent: to_iced(palette.urgent),
        warning: to_iced(palette.warning),
        focus: to_iced(palette.focus),
        dim: to_iced(palette.dim),
    }
}

fn to_iced(c: DynamicColor) -> Color {
    let [r, g, b, a] = c
        .convert(ColorSpaceTag::Srgb)
        .to_alpha_color::<Srgb>()
        .components;
    Color { r, g, b, a }
}

fn resolve_font(config: &DiskFontConfig) -> ResolvedFontConfig {
    let name = config
        .name
        .clone()
        .or_else(|| fc_match("sans-serif", "%{family}"))
        .unwrap_or_else(|| "sans-serif".to_owned());
    // Read the resolved family's actual file so it can be registered with iced
    // (see `data`). iced's cosmic-text DB only resolves `Font::with_name`
    // against faces it was explicitly handed; without the real file, bar text
    // falls back to the Font Awesome icon font (tofu).
    let bytes = fc_match(&name, "%{file}")
        .and_then(|path| std::fs::read(path).ok())
        .expect("no readable sans-serif font found");
    let face = ttf_parser::Face::parse(&bytes, 0).expect("sans-serif font failed to parse");
    let descent_px =
        f32::from(face.descender().unsigned_abs()) / f32::from(face.units_per_em())
            * config.text_size_px;
    let covered = covered_chars(&face);
    // The one unavoidable leak: iced's `Font::with_name` requires `&'static`.
    let iced_font = Font::with_name(String::leak(name));
    ResolvedFontConfig {
        iced_font,
        text_size_px: config.text_size_px,
        icon_size_px: config.icon_size_px,
        data: Arc::from(bytes),
        descent_px,
        covered: Arc::new(covered),
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

/// Query one field (`format`, e.g. `%{family}` or `%{file}`) of fontconfig's
/// best match for `pattern` via `fc-match`, which reads libfontconfig's prebuilt
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
    // fontconfig may emit a comma-separated list (e.g. family aliases); take the
    // first.
    let value = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
