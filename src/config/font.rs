use std::{collections::HashSet, fs, process::Command, sync::Arc};

use iced::Font;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UserFont {
    pub name: Option<String>,
    pub text_size_px: f32,
    pub icon_size_px: f32,
}

impl Default for UserFont {
    fn default() -> Self {
        Self {
            name: None,
            text_size_px: 14.0,
            icon_size_px: 16.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FontConfig {
    pub iced_font: Font,
    pub text_size_px: f32,
    pub icon_size_px: f32,
    pub data: Arc<[u8]>,
    pub covered: Arc<HashSet<char>>,
}

pub(super) fn resolve(config: &UserFont) -> FontConfig {
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
    FontConfig {
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
