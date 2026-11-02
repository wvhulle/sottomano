use std::{fmt, fs, io, process::Command, sync::Arc};

use serde::{Deserialize, Serialize};
use ttf_parser::FaceParsingError;

use crate::spacing::Spacing;

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
    pub(crate) fn text_size_px(&self) -> Result<f32, FontError> {
        match self.text_size {
            Spacing::Px(px) => Ok(px),
            relative @ Spacing::Em(_) => Err(FontError::RelativeTextSize(relative)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedFont {
    pub role: FontRole,
    pub family: String,
    pub text_size_px: f32,
    pub small_text_size_px: f32,
    pub icon_size_px: f32,
    pub data: Arc<[u8]>,
}

impl ResolvedFont {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontRole {
    Text,
    Monospace,
}

pub(crate) fn resolve(config: &FontChoice, role: FontRole) -> Result<ResolvedFont, FontError> {
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
    ttf_parser::Face::parse(&bytes, 0).map_err(|error| FontError::Unparsable {
        file: matched.file.clone(),
        error,
    })?;
    let text_size_px = config.text_size_px()?;
    Ok(ResolvedFont {
        role,
        family: configured.map_or(matched.family, str::to_owned),
        text_size_px,
        small_text_size_px: config.small_text_size.resolve(text_size_px),
        icon_size_px: config.icon_size.resolve(text_size_px),
        data: Arc::from(bytes),
    })
}

fn fc_match_stdout(pattern: &str, format: &str) -> Option<String> {
    let output = Command::new("fc-match")
        .args(["-f", format, pattern])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

struct FontMatch {
    family: String,
    file: String,
}

fn fc_match_family_file(pattern: &str) -> Option<FontMatch> {
    let stdout = fc_match_stdout(pattern, "%{family}\t%{file}")?;
    let line = stdout.lines().next()?;
    let (family, file) = line.split_once('\t')?;
    let family = family.split(',').next()?.trim();
    (!family.is_empty() && !file.is_empty()).then(|| FontMatch {
        family: family.to_owned(),
        file: file.trim().to_owned(),
    })
}
