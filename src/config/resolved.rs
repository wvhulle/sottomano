//! Config resolved once at startup: the font is loaded and parsed, colours are
//! converted. Fields are plain data — read them directly.

use std::{collections::HashSet, sync::Arc};

use color::{ColorSpaceTag, DynamicColor, Srgb};
use iced::{Color, Font};

use super::disk::{
    self, DiskConfig, DiskFontConfig, DiskPaletteConfig, IdleConfig, Layout, NotificationConfig,
};

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub palette: ResolvedPaletteConfig,
    pub layout: Layout,
    pub font: ResolvedFontConfig,
    /// Passed through from disk unchanged: notification settings need no
    /// font/colour resolution, and the fallback corner is already an enum.
    pub notifications: NotificationConfig,
    /// Float inset in px — pure persisted UI state, not a `vetrata.toml` knob.
    /// Starts at 0 and is loaded from the sqlite state DB (`state::panel`) at
    /// startup, then mutated live by the panel-settings slider.
    pub float_offset_px: f32,
    /// Command per idle subsystem, for the in-process idle daemon.
    pub idle_bindings: IdleBindings,
}

/// The command (argv) to spawn when each idle subsystem fires. Empty = arm but
/// do nothing.
#[derive(Debug, Clone, Default)]
pub struct IdleBindings {
    pub lock_user: Vec<String>,
    pub screen_off: Vec<String>,
    pub standby: Vec<String>,
    pub power_down: Vec<String>,
}

impl ResolvedConfig {
    /// Corner radius to draw: the configured radius when floating, else `0`.
    /// Derived from the live layout rather than resolved once, so it follows
    /// a runtime floating toggle.
    pub fn chrome_radius(&self) -> f32 {
        if self.layout.floating {
            f32::from(self.layout.corner_radius_px)
        } else {
            0.0
        }
    }
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

#[derive(Debug, Clone)]
pub struct ResolvedFontConfig {
    /// Leaked once at startup: the family name must be `'static` for the
    /// program's lifetime.
    pub iced_font: Font,
    pub text_size_px: f32,
    pub icon_size_px: f32,
    pub data: Arc<[u8]>,
    /// Font descent in px at the bar text size.
    pub descent_px: f32,
    /// The codepoints the font has glyphs for.
    pub covered: Arc<HashSet<char>>,
}

/// Read the config from disk and resolve it. Panics if no sans-serif font can
/// be loaded at all. Call once, at startup.
pub fn load() -> ResolvedConfig {
    let path = disk::config_path();
    let user = match disk::read(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {} {e}; using defaults", path.display());
            DiskConfig::default()
        }
    };
    ResolvedConfig {
        palette: resolve_palette(&user.palette),
        layout: user.layout,
        font: resolve_font(&user.font),
        notifications: user.notifications,
        // Overwritten by the persisted value once `state::panel` loads.
        float_offset_px: 0.0,
        idle_bindings: resolve_idle_bindings(&user.idle),
    }
}

fn resolve_idle_bindings(config: &IdleConfig) -> IdleBindings {
    let binding = |name: &str| {
        config
            .bindings
            .get(name)
            .map(|cmd| cmd.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    };
    IdleBindings {
        lock_user: binding("lock-user"),
        screen_off: binding("screen-off"),
        standby: binding("standby"),
        power_down: binding("power-down"),
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
    let bytes = fc_match(&name, "%{file}")
        .and_then(|path| std::fs::read(path).ok())
        .expect("no readable sans-serif font found");
    let face = ttf_parser::Face::parse(&bytes, 0).expect("sans-serif font failed to parse");
    let descent_px = f32::from(face.descender().unsigned_abs()) / f32::from(face.units_per_em())
        * config.text_size_px;
    let covered = covered_chars(&face);
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

/// The family fontconfig picks for a single codepoint. Spawns a subprocess;
/// call off the render path and cache the result.
pub fn fallback_font(ch: char) -> Option<Font> {
    fc_match(&format!(":charset={:x}", ch as u32), "%{family}")
        .map(|family| Font::with_name(String::leak(family)))
}

/// One field of fontconfig's best match for `pattern`, via the `fc-match`
/// subprocess.
fn fc_match(pattern: &str, format: &str) -> Option<String> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", format, pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    // fontconfig may return a comma-separated alias list; take the first.
    let value = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
