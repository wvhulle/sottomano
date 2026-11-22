use std::path::Path;

use freedesktop_desktop_entry::GenericEntry;

use super::{DirectorySizing, ThemeIndexError};

const ICON_THEME_SECTION: &str = "Icon Theme";
const DEFAULT_THRESHOLD_PX: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum DirectorySizingError {
    #[error("declares no Size")]
    UndeclaredSize,
    #[error("{key} is not a pixel count: {value}")]
    InvalidPixels { key: &'static str, value: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclaredPixels {
    Undeclared,
    Declared(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectoryDeclaration {
    pub(super) relative_path: String,
    pub(super) sizing: DirectorySizing,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct ThemeMetadata {
    pub(super) inherited_themes: Vec<String>,
    pub(super) directory_declarations: Vec<DirectoryDeclaration>,
}

pub(super) fn parsed_theme_metadata(
    index_file: &Path,
    index_text: &str,
) -> Result<ThemeMetadata, ThemeIndexError> {
    let theme_file = GenericEntry::from_str(index_file, index_text).map_err(|decode_failure| {
        ThemeIndexError::MalformedThemeFile {
            file: index_file.to_path_buf(),
            decode_failure,
        }
    })?;
    let theme_entry = |group_name: &str, key: &str| -> Option<String> {
        theme_file
            .groups
            .group(group_name)
            .and_then(|group| group.entry(key))
            .map(str::to_owned)
    };
    let directory_declarations = listed_values(theme_entry(ICON_THEME_SECTION, "Directories"))
        .into_iter()
        .filter(|relative_path| {
            theme_entry(relative_path, "Scale").is_none_or(|scale| scale == "1")
        })
        .filter_map(|relative_path| {
            match directory_sizing(&|key| theme_entry(&relative_path, key)) {
                Ok(sizing) => Some(DirectoryDeclaration {
                    relative_path,
                    sizing,
                }),
                Err(sizing_failure) => {
                    tracing::debug!(
                        index_file = %index_file.display(),
                        relative_path,
                        %sizing_failure,
                        "icon theme: skipping a directory"
                    );
                    None
                }
            }
        })
        .collect();
    Ok(ThemeMetadata {
        inherited_themes: listed_values(theme_entry(ICON_THEME_SECTION, "Inherits")),
        directory_declarations,
    })
}

fn listed_values(declared_list: Option<String>) -> Vec<String> {
    declared_list.map_or_else(Vec::new, |list_text| {
        list_text
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_owned)
            .collect()
    })
}

fn declared_pixels(
    directory_entry: &dyn Fn(&'static str) -> Option<String>,
    key: &'static str,
) -> Result<DeclaredPixels, DirectorySizingError> {
    let Some(value) = directory_entry(key) else {
        return Ok(DeclaredPixels::Undeclared);
    };
    value
        .trim()
        .parse()
        .map(DeclaredPixels::Declared)
        .map_err(|_| DirectorySizingError::InvalidPixels { key, value })
}

fn directory_sizing(
    directory_entry: &dyn Fn(&'static str) -> Option<String>,
) -> Result<DirectorySizing, DirectorySizingError> {
    let DeclaredPixels::Declared(side_px) = declared_pixels(directory_entry, "Size")? else {
        return Err(DirectorySizingError::UndeclaredSize);
    };
    let pixels_or_side = |key| -> Result<u16, DirectorySizingError> {
        Ok(match declared_pixels(directory_entry, key)? {
            DeclaredPixels::Declared(pixels) => pixels,
            DeclaredPixels::Undeclared => side_px,
        })
    };
    Ok(match directory_entry("Type").as_deref() {
        Some("Fixed") => DirectorySizing::Fixed { side_px },
        Some("Scalable") => DirectorySizing::Scalable {
            min_px: pixels_or_side("MinSize")?,
            max_px: pixels_or_side("MaxSize")?,
        },
        Some(_) | None => DirectorySizing::Threshold {
            side_px,
            threshold_px: match declared_pixels(directory_entry, "Threshold")? {
                DeclaredPixels::Declared(threshold_px) => threshold_px,
                DeclaredPixels::Undeclared => DEFAULT_THRESHOLD_PX,
            },
        },
    })
}
