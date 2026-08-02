use std::{collections::HashSet, env, fs, path::PathBuf, process::Command, sync::Arc};

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
    pub notifications: NotificationConfig,
    pub float_offset_px: f32,
    pub darken_chrome_factor: f32,
    pub dnd: bool,
    pub sound: bool,
    pub sound_theme: Option<String>,
    pub tray_deny_ids: Vec<String>,
    pub terminal_command_prefix: Vec<String>,
    pub secrets: SecretPaths,
}

#[derive(Debug, Clone, Default)]
pub struct SecretPaths {
    pub openrouter_management: Option<PathBuf>,
    pub claude_code_oauth: Option<PathBuf>,
    pub gh_auth_token: Option<PathBuf>,
}

impl ResolvedConfig {
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
    pub iced_font: Font,
    pub text_size_px: f32,
    pub icon_size_px: f32,
    pub data: Arc<[u8]>,
    pub covered: Arc<HashSet<char>>,
}

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
        float_offset_px: 0.0,
        darken_chrome_factor: 0.0,
        dnd: false,
        sound: true,
        sound_theme: None,
        tray_deny_ids: user
            .tray
            .deny_ids
            .iter()
            .map(|id| id.to_lowercase())
            .collect(),
        terminal_command_prefix: user.terminal,
        secrets: SecretPaths {
            openrouter_management: secret_path("VETRATA_OPENROUTER_KEY_FILE"),
            claude_code_oauth: secret_path("VETRATA_CLAUDE_CODE_OAUTH_FILE"),
            gh_auth_token: secret_path("VETRATA_GH_AUTH_TOKEN_FILE"),
        },
    }
}

fn secret_path(var: &str) -> Option<PathBuf> {
    env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn session_sound_theme() -> Option<String> {
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

fn to_iced(dynamic: DynamicColor) -> Color {
    let [r, g, b, a] = dynamic
        .convert(ColorSpaceTag::Srgb)
        .to_alpha_color::<Srgb>()
        .components;
    Color { r, g, b, a }
}

fn resolve_font(config: &DiskFontConfig) -> ResolvedFontConfig {
    let matched = fc_match_family_file(config.name.as_deref().unwrap_or("sans-serif"));
    let name = config
        .name
        .clone()
        .or_else(|| matched.as_ref().map(|m| m.family.clone()))
        .unwrap_or_else(|| "sans-serif".to_owned());
    let bytes = matched
        .and_then(|m| fs::read(m.file).ok())
        .expect("no readable sans-serif font found");
    let face = ttf_parser::Face::parse(&bytes, 0).expect("sans-serif font failed to parse");
    let covered = covered_chars(&face);
    let iced_font = Font::with_name(String::leak(name));
    ResolvedFontConfig {
        iced_font,
        text_size_px: config.text_size_px,
        icon_size_px: config.icon_size_px,
        data: Arc::from(bytes),
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

pub fn fallback_font(ch: char) -> Option<Font> {
    fc_match(&format!(":charset={:x}", ch as u32), "%{family}")
        .map(|family| Font::with_name(String::leak(family)))
}

fn fc_match(pattern: &str, format: &str) -> Option<String> {
    let output = Command::new("fc-match")
        .args(["-f", format, pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let value = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

struct FontMatch {
    family: String,
    file: String,
}

fn fc_match_family_file(pattern: &str) -> Option<FontMatch> {
    let output = Command::new("fc-match")
        .args(["-f", "%{family}\t%{file}", pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let line = stdout.lines().next()?;
    let (family, file) = line.split_once('\t')?;
    let family = family.split(',').next()?.trim();
    (!family.is_empty() && !file.is_empty()).then(|| FontMatch {
        family: family.to_owned(),
        file: file.trim().to_owned(),
    })
}
