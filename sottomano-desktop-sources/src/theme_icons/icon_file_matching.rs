use std::path::PathBuf;

use crate::theme_icons::theme_directory_index::{
    DirectorySizing, IndexedTheme, SizedIconDirectory, ThemeDirectoryIndex,
};

pub const SYMBOLIC_ICON_SUFFIX: &str = "-symbolic";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameVariants {
    SymbolicFirst,
    AsNamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconFileMatch {
    Found(PathBuf),
    Missing,
}

#[must_use]
pub fn candidate_icon_names(icon_name: &str, name_variants: NameVariants) -> Vec<String> {
    match name_variants {
        NameVariants::SymbolicFirst if !icon_name.ends_with(SYMBOLIC_ICON_SUFFIX) => vec![
            format!("{icon_name}{SYMBOLIC_ICON_SUFFIX}"),
            icon_name.to_owned(),
        ],
        NameVariants::SymbolicFirst | NameVariants::AsNamed => vec![icon_name.to_owned()],
    }
}

#[must_use]
pub fn matching_icon_file(
    theme_index: &ThemeDirectoryIndex,
    icon_name: &str,
    wanted_side_px: u16,
) -> IconFileMatch {
    theme_index
        .searched_themes
        .iter()
        .find_map(|indexed_theme| best_file_in_theme(indexed_theme, icon_name, wanted_side_px))
        .or_else(|| theme_index.unthemed_pixmaps.get(icon_name).cloned())
        .map_or(IconFileMatch::Missing, IconFileMatch::Found)
}

fn best_file_in_theme(
    indexed_theme: &IndexedTheme,
    icon_name: &str,
    wanted_side_px: u16,
) -> Option<PathBuf> {
    indexed_theme
        .sized_directories
        .iter()
        .filter_map(|sized_directory| {
            sized_directory
                .icon_files
                .get(icon_name)
                .map(|path| SizedCandidate {
                    path,
                    size_distance_px: size_distance_px(sized_directory, wanted_side_px),
                })
        })
        .min_by_key(|candidate| candidate.size_distance_px)
        .map(|candidate| candidate.path.clone())
}

struct SizedCandidate<'index> {
    path: &'index PathBuf,
    size_distance_px: u32,
}

fn size_distance_px(sized_directory: &SizedIconDirectory, wanted_side_px: u16) -> u32 {
    let wanted_px = i32::from(wanted_side_px);
    let distance_outside = |lowest_px: i32, highest_px: i32| {
        if wanted_px < lowest_px {
            lowest_px - wanted_px
        } else if wanted_px > highest_px {
            wanted_px - highest_px
        } else {
            0
        }
    };
    let distance_px = match sized_directory.sizing {
        DirectorySizing::Fixed { side_px } => i32::from(side_px) - wanted_px,
        DirectorySizing::Scalable { min_px, max_px } => {
            distance_outside(i32::from(min_px), i32::from(max_px))
        }
        DirectorySizing::Threshold {
            side_px,
            threshold_px,
        } => distance_outside(
            i32::from(side_px) - i32::from(threshold_px),
            i32::from(side_px) + i32::from(threshold_px),
        ),
    };
    distance_px.unsigned_abs()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn sized_directory(sizing: DirectorySizing, icon_name: &str) -> SizedIconDirectory {
        SizedIconDirectory {
            sizing,
            icon_files: HashMap::from([(
                icon_name.to_owned(),
                PathBuf::from(format!("/{sizing:?}/{icon_name}.svg")),
            )]),
        }
    }

    fn theme_with(theme_name: &str, sized_directories: Vec<SizedIconDirectory>) -> IndexedTheme {
        IndexedTheme {
            theme_name: theme_name.to_owned(),
            sized_directories,
        }
    }

    fn example_then_hicolor() -> ThemeDirectoryIndex {
        ThemeDirectoryIndex {
            searched_themes: vec![
                theme_with(
                    "Example-Dark",
                    vec![
                        sized_directory(DirectorySizing::Fixed { side_px: 16 }, "edit-select"),
                        sized_directory(DirectorySizing::Fixed { side_px: 32 }, "edit-select"),
                    ],
                ),
                theme_with(
                    "hicolor",
                    vec![sized_directory(
                        DirectorySizing::Scalable {
                            min_px: 16,
                            max_px: 512,
                        },
                        "firefox",
                    )],
                ),
            ],
            unthemed_pixmaps: HashMap::from([(
                "xterm".to_owned(),
                PathBuf::from("/pixmaps/xterm.xpm"),
            )]),
        }
    }

    #[test]
    fn the_closest_design_size_wins_within_a_theme() {
        assert_eq!(
            matching_icon_file(&example_then_hicolor(), "edit-select", 18),
            IconFileMatch::Found(PathBuf::from("/Fixed { side_px: 16 }/edit-select.svg"))
        );
        assert_eq!(
            matching_icon_file(&example_then_hicolor(), "edit-select", 30),
            IconFileMatch::Found(PathBuf::from("/Fixed { side_px: 32 }/edit-select.svg"))
        );
    }

    #[test]
    fn inherited_themes_and_pixmaps_are_searched_in_order() {
        assert!(matches!(
            matching_icon_file(&example_then_hicolor(), "firefox", 48),
            IconFileMatch::Found(_)
        ));
        assert_eq!(
            matching_icon_file(&example_then_hicolor(), "xterm", 48),
            IconFileMatch::Found(PathBuf::from("/pixmaps/xterm.xpm"))
        );
        assert_eq!(
            matching_icon_file(&example_then_hicolor(), "absent-icon", 16),
            IconFileMatch::Missing
        );
    }

    #[test]
    fn symbolic_variants_are_tried_first_only_when_asked() {
        assert_eq!(
            candidate_icon_names("edit-select", NameVariants::SymbolicFirst),
            ["edit-select-symbolic", "edit-select"]
        );
        assert_eq!(
            candidate_icon_names("edit-select-symbolic", NameVariants::SymbolicFirst),
            ["edit-select-symbolic"]
        );
        assert_eq!(
            candidate_icon_names("edit-select", NameVariants::AsNamed),
            ["edit-select"]
        );
    }
}
