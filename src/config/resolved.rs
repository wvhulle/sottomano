//! Config resolved once at startup: the font is loaded and parsed, colours are
//! converted. Fields are plain data — read them directly.

use std::{collections::HashSet, process::Command, sync::Arc};

use color::{ColorSpaceTag, DynamicColor, Srgb};
use iced::{Color, Font};

use super::disk::{
    self, DiskConfig, DiskFontConfig, DiskPaletteConfig, Layout, NotificationConfig,
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
    /// Do-not-disturb: suppress and discard all notification toasts. The single
    /// home for this state — the notification layer and the bell widget both
    /// read it here. Persisted UI state, not a `vetrata.toml` knob: starts
    /// `false`, loaded from the state DB at startup, toggled live from the
    /// notification group.
    pub dnd: bool,
    /// Whether an alert sound plays on each incoming notification. Persisted UI
    /// state; starts `true` (sounds on), loaded from the state DB at startup,
    /// toggled live from the notification group.
    pub sound: bool,
    /// The session's configured XDG sound theme (`org.gnome.desktop.sound
    /// theme-name`), resolved once at startup — the sound analogue of the
    /// gsettings icon-theme pick. `None` (no gsettings / unset) leaves the
    /// sound lookup on its "freedesktop" default.
    pub sound_theme: Option<String>,
    /// Lowercased SNI id substrings whose tray items are hidden (already
    /// surfaced natively by the shell). See `disk::TrayConfig::deny_ids`.
    pub tray_deny_ids: Vec<String>,
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
        // Overwritten by the persisted values once `state::panel` loads.
        float_offset_px: 0.0,
        dnd: false,
        sound: true,
        sound_theme: session_sound_theme(),
        tray_deny_ids: user
            .tray
            .deny_ids
            .iter()
            .map(|id| id.to_lowercase())
            .collect(),
    }
}

/// The session's configured sound theme from gsettings
/// (`org.gnome.desktop.sound theme-name`), so notification alert sounds follow
/// the desktop's choice — the audio analogue of the icon-theme pick. gsettings
/// quotes the value (`'ocean'`), so the surrounding quotes are trimmed. `None`
/// when gsettings is absent or the key is unset, leaving the sound lookup on
/// its "freedesktop" default.
fn session_sound_theme() -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.sound", "theme-name"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let theme = value.trim().trim_matches('\'');
    (!theme.is_empty()).then(|| theme.to_owned())
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
    let output = Command::new("fc-match")
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
