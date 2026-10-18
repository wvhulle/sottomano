use std::{collections::HashSet, fmt, fs, io, sync::Arc};

use iced::{
    Font,
    font::{Style, Weight},
};
use serde::{Deserialize, Serialize};
use ttf_parser::FaceParsingError;

use crate::{config::layout::spacing::Spacing, data::source::command};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontChoice {
    pub name: Option<String>,
    pub monospace_name: Option<String>,
    pub text_size: Spacing,
    pub small_text_size: Spacing,
    pub icon_size: Spacing,
}

impl Default for FontChoice {
    fn default() -> Self {
        Self {
            name: None,
            monospace_name: None,
            text_size: Spacing::Px(14.0),
            small_text_size: Spacing::Px(12.0),
            icon_size: Spacing::Px(15.5),
        }
    }
}

impl FontChoice {
    pub(in crate::config) fn text_size_px(&self) -> Result<f32, FontError> {
        match self.text_size {
            Spacing::Px(px) => Ok(px),
            relative @ Spacing::Em(_) => Err(FontError::RelativeTextSize(relative)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedFont {
    pub iced_font: Font,
    pub text_size_px: f32,
    pub small_text_size_px: f32,
    pub icon_size_px: f32,
    pub data: Arc<[u8]>,
    pub covered: Arc<HashSet<char>>,
}

impl ResolvedFont {
    #[must_use]
    pub fn bold(&self) -> Font {
        Font {
            weight: Weight::Bold,
            ..self.iced_font
        }
    }

    #[must_use]
    pub fn italic(&self) -> Font {
        Font {
            style: Style::Italic,
            ..self.iced_font
        }
    }

    #[must_use]
    pub fn has_same_face_as(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug)]
pub enum FontError {
    RelativeTextSize(Spacing),
    Unmatched(String),
    Unreadable {
        file: String,
        error: io::Error,
    },
    Unparsable {
        file: String,
        error: FaceParsingError,
    },
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RelativeTextSize(size) => write!(
                f,
                "font text_size {size} is relative; the base text size must be given in px"
            ),
            Self::Unmatched(pattern) => write!(f, "fc-match found no font file for {pattern:?}"),
            Self::Unreadable { file, error } => {
                write!(f, "font file {file} cannot be read: {error}")
            }
            Self::Unparsable { file, error } => {
                write!(f, "font file {file} cannot be parsed: {error}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::config) enum FontRole {
    Text,
    Monospace,
}

pub(in crate::config) fn resolve(
    config: &FontChoice,
    role: FontRole,
) -> Result<ResolvedFont, FontError> {
    let configured = match role {
        FontRole::Text => config.name.as_deref(),
        FontRole::Monospace => config.monospace_name.as_deref(),
    };
    let generic = match role {
        FontRole::Text => "sans-serif",
        FontRole::Monospace => "monospace",
    };
    let pattern = configured.unwrap_or(generic);
    let matched =
        fc_match_family_file(pattern).ok_or_else(|| FontError::Unmatched(pattern.to_owned()))?;
    let bytes = fs::read(&matched.file).map_err(|error| FontError::Unreadable {
        file: matched.file.clone(),
        error,
    })?;
    let face = ttf_parser::Face::parse(&bytes, 0).map_err(|error| FontError::Unparsable {
        file: matched.file.clone(),
        error,
    })?;
    let covered = covered_chars(&face);
    let name = configured.map_or(matched.family, str::to_owned);
    let text_size_px = config.text_size_px()?;
    Ok(ResolvedFont {
        iced_font: Font::with_name(String::leak(name)),
        text_size_px,
        small_text_size_px: config.small_text_size.resolve(text_size_px),
        icon_size_px: config.icon_size.resolve(text_size_px),
        data: Arc::from(bytes),
        covered: Arc::new(covered),
    })
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

#[must_use]
pub fn fallback_font(ch: char) -> Option<Font> {
    fc_match(&format!(":charset={:x}", ch as u32), "%{family}")
        .map(|family| Font::with_name(String::leak(family)))
}

fn fc_match(pattern: &str, format: &str) -> Option<String> {
    let stdout = command::blocking::output("fc-match", &["-f", format, pattern]).ok()?;
    let value = stdout.lines().next()?.split(',').next()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

struct FontMatch {
    family: String,
    file: String,
}

fn fc_match_family_file(pattern: &str) -> Option<FontMatch> {
    let stdout =
        command::blocking::output("fc-match", &["-f", "%{family}\t%{file}", pattern]).ok()?;
    let line = stdout.lines().next()?;
    let (family, file) = line.split_once('\t')?;
    let family = family.split(',').next()?.trim();
    (!family.is_empty() && !file.is_empty()).then(|| FontMatch {
        family: family.to_owned(),
        file: file.trim().to_owned(),
    })
}
