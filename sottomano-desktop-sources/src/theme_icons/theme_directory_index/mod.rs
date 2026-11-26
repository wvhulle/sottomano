mod index_theme_metadata;
#[cfg(test)]
mod theme_directory_index_tests;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, io,
    path::{Path, PathBuf},
};

use freedesktop_desktop_entry::DecodeError;
use futures::{StreamExt, stream};
use index_theme_metadata::{ThemeMetadata, parsed_theme_metadata};
use tokio::fs;

pub const FALLBACK_THEME_NAME: &str = "hicolor";
const DEFAULT_DATA_DIRECTORIES: &str = "/usr/local/share:/usr/share";
const ICON_FILE_EXTENSIONS: [&str; 3] = ["png", "svg", "xpm"];
const DIRECTORY_LISTINGS_IN_FLIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorySizing {
    Fixed { side_px: u16 },
    Scalable { min_px: u16, max_px: u16 },
    Threshold { side_px: u16, threshold_px: u16 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SizedIconDirectory {
    pub sizing: DirectorySizing,
    pub icon_files: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexedTheme {
    pub theme_name: String,
    pub sized_directories: Vec<SizedIconDirectory>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeDirectoryIndex {
    pub searched_themes: Vec<IndexedTheme>,
    pub unthemed_pixmaps: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconSearchRoots {
    pub theme_roots: Vec<PathBuf>,
    pub pixmap_directories: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeIndexError {
    #[error("cannot read icon theme file {file}: {read_failure}")]
    UnreadableThemeFile {
        file: PathBuf,
        read_failure: io::Error,
    },
    #[error("malformed icon theme file {file}: {decode_failure}")]
    MalformedThemeFile {
        file: PathBuf,
        decode_failure: DecodeError,
    },
    #[error("cannot list icon directory {directory}: {list_failure}")]
    UnlistableDirectory {
        directory: PathBuf,
        list_failure: io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ThemeMetadataLookup {
    Declared(ThemeMetadata),
    NotInstalled,
}

struct DeclaredTheme {
    theme_name: String,
    metadata: ThemeMetadata,
}

struct PlannedListing {
    theme_position: usize,
    directory_position: usize,
    directory: PathBuf,
}

struct CompletedListing {
    planned_listing: PlannedListing,
    listed_files: Result<Vec<ListedIconFile>, ThemeIndexError>,
}

struct ListedIconFile {
    icon_name: String,
    path: PathBuf,
}

#[must_use]
pub fn xdg_icon_search_roots() -> IconSearchRoots {
    let home_directory = env::var_os("HOME").map(PathBuf::from);
    let data_home = env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
        home_directory
            .as_ref()
            .map(|home| home.join(".local/share"))
    });
    let data_directories_text =
        env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| DEFAULT_DATA_DIRECTORIES.into());
    let data_directories: Vec<PathBuf> = env::split_paths(&data_directories_text).collect();
    IconSearchRoots {
        theme_roots: home_directory
            .iter()
            .map(|home| home.join(".icons"))
            .chain(data_home.iter().map(|data| data.join("icons")))
            .chain(data_directories.iter().map(|data| data.join("icons")))
            .collect(),
        pixmap_directories: data_directories
            .iter()
            .map(|data| data.join("pixmaps"))
            .collect(),
    }
}

pub async fn indexed_icon_theme(
    theme_name: String,
    search_roots: IconSearchRoots,
) -> Result<ThemeDirectoryIndex, ThemeIndexError> {
    let declared_themes = declared_theme_chain(theme_name, &search_roots.theme_roots).await?;
    let mut planned_listings = Vec::new();
    for (theme_position, declared_theme) in declared_themes.iter().enumerate() {
        let theme_directories =
            installed_theme_directories(&declared_theme.theme_name, &search_roots.theme_roots)
                .await?;
        for (directory_position, declaration) in declared_theme
            .metadata
            .directory_declarations
            .iter()
            .enumerate()
        {
            for theme_directory in &theme_directories {
                planned_listings.push(PlannedListing {
                    theme_position,
                    directory_position,
                    directory: theme_directory.join(&declaration.relative_path),
                });
            }
        }
    }
    let completed_listings: Vec<CompletedListing> = stream::iter(planned_listings)
        .map(|planned_listing| async move {
            let listed_files = listed_icon_files(&planned_listing.directory).await;
            CompletedListing {
                planned_listing,
                listed_files,
            }
        })
        .buffered(DIRECTORY_LISTINGS_IN_FLIGHT)
        .collect()
        .await;
    let mut searched_themes: Vec<IndexedTheme> = declared_themes
        .into_iter()
        .map(|declared_theme| IndexedTheme {
            theme_name: declared_theme.theme_name,
            sized_directories: declared_theme
                .metadata
                .directory_declarations
                .into_iter()
                .map(|declaration| SizedIconDirectory {
                    sizing: declaration.sizing,
                    icon_files: HashMap::new(),
                })
                .collect(),
        })
        .collect();
    for completed_listing in completed_listings {
        let PlannedListing {
            theme_position,
            directory_position,
            ..
        } = completed_listing.planned_listing;
        let Some(sized_directory) = searched_themes
            .get_mut(theme_position)
            .and_then(|indexed_theme| indexed_theme.sized_directories.get_mut(directory_position))
        else {
            continue;
        };
        for listed_file in completed_listing.listed_files? {
            sized_directory
                .icon_files
                .entry(listed_file.icon_name)
                .or_insert(listed_file.path);
        }
    }
    let unthemed_pixmaps = first_listed_paths(
        stream::iter(search_roots.pixmap_directories)
            .map(|pixmap_directory| async move { listed_icon_files(&pixmap_directory).await })
            .buffered(DIRECTORY_LISTINGS_IN_FLIGHT)
            .collect()
            .await,
    )?;
    Ok(ThemeDirectoryIndex {
        searched_themes,
        unthemed_pixmaps,
    })
}

async fn installed_theme_directories(
    theme_name: &str,
    theme_roots: &[PathBuf],
) -> Result<Vec<PathBuf>, ThemeIndexError> {
    let mut theme_directories = Vec::new();
    for theme_root in theme_roots {
        let theme_directory = theme_root.join(theme_name);
        match fs::metadata(&theme_directory).await {
            Ok(metadata) if metadata.is_dir() => theme_directories.push(theme_directory),
            Ok(_) => {}
            Err(probe_failure) if probe_failure.kind() == io::ErrorKind::NotFound => {}
            Err(list_failure) => {
                return Err(ThemeIndexError::UnlistableDirectory {
                    directory: theme_directory,
                    list_failure,
                });
            }
        }
    }
    Ok(theme_directories)
}

async fn declared_theme_chain(
    theme_name: String,
    theme_roots: &[PathBuf],
) -> Result<Vec<DeclaredTheme>, ThemeIndexError> {
    let mut queued_themes = VecDeque::from([theme_name, FALLBACK_THEME_NAME.to_owned()]);
    let mut visited_themes = HashSet::new();
    let mut declared_themes = Vec::new();
    while let Some(next_theme) = queued_themes.pop_front() {
        if !visited_themes.insert(next_theme.clone()) {
            continue;
        }
        match theme_metadata(&next_theme, theme_roots).await? {
            ThemeMetadataLookup::Declared(metadata) => {
                metadata
                    .inherited_themes
                    .iter()
                    .rev()
                    .for_each(|inherited_theme| queued_themes.push_front(inherited_theme.clone()));
                declared_themes.push(DeclaredTheme {
                    theme_name: next_theme,
                    metadata,
                });
            }
            ThemeMetadataLookup::NotInstalled => {
                tracing::debug!(theme_name = %next_theme, "icon theme: not installed");
            }
        }
    }
    Ok(declared_themes)
}

async fn theme_metadata(
    theme_name: &str,
    theme_roots: &[PathBuf],
) -> Result<ThemeMetadataLookup, ThemeIndexError> {
    for theme_root in theme_roots {
        let index_file = theme_root.join(theme_name).join("index.theme");
        match fs::read_to_string(&index_file).await {
            Ok(index_text) => {
                return Ok(ThemeMetadataLookup::Declared(parsed_theme_metadata(
                    &index_file,
                    &index_text,
                )?));
            }
            Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => {}
            Err(read_failure) => {
                return Err(ThemeIndexError::UnreadableThemeFile {
                    file: index_file,
                    read_failure,
                });
            }
        }
    }
    Ok(ThemeMetadataLookup::NotInstalled)
}

fn first_listed_paths(
    listed_directories: Vec<Result<Vec<ListedIconFile>, ThemeIndexError>>,
) -> Result<HashMap<String, PathBuf>, ThemeIndexError> {
    listed_directories
        .into_iter()
        .try_fold(HashMap::new(), |mut first_paths, listed_files| {
            listed_files?.into_iter().for_each(|listed_file| {
                first_paths
                    .entry(listed_file.icon_name)
                    .or_insert(listed_file.path);
            });
            Ok(first_paths)
        })
}

async fn listed_icon_files(directory: &Path) -> Result<Vec<ListedIconFile>, ThemeIndexError> {
    let unlistable = |list_failure| ThemeIndexError::UnlistableDirectory {
        directory: directory.to_path_buf(),
        list_failure,
    };
    let mut directory_entries = match fs::read_dir(directory).await {
        Err(list_failure) if list_failure.kind() == io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        opened => opened.map_err(unlistable)?,
    };
    let mut listed_files = Vec::new();
    while let Some(directory_entry) = directory_entries.next_entry().await.map_err(unlistable)? {
        let path = directory_entry.path();
        let has_icon_extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| ICON_FILE_EXTENSIONS.contains(&extension));
        let Some(icon_name) = path.file_stem().and_then(|file_stem| file_stem.to_str()) else {
            continue;
        };
        if has_icon_extension {
            listed_files.push(ListedIconFile {
                icon_name: icon_name.to_owned(),
                path,
            });
        }
    }
    Ok(listed_files)
}
