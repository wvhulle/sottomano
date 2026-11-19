use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::{StreamExt, stream};
use tokio::fs;

use crate::{
    desktop_windows_apps::{
        desktop_entries::{AppIconCandidate, DesktopEntries},
        window_title_labels::IconCandidate,
        workspace_window_snapshot::{Snapshot, WinEntry},
    },
    theme_icons::{
        icon_file_loading::{IconLoadError, IconTrim, LoadedIcon, loaded_icon_file},
        icon_file_matching::{
            IconFileMatch, NameVariants, candidate_icon_names, matching_icon_file,
        },
        theme_directory_index::ThemeDirectoryIndex,
    },
};

pub const APP_ICON_SIDE_PX: u16 = 48;
const ICON_LOADS_IN_FLIGHT: usize = 16;
const ITEM_THEME_PATH_EXTENSIONS: [&str; 2] = ["svg", "png"];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemThemePath {
    Unset,
    Directory(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconRequest {
    AppIcon {
        app_id: String,
    },
    NamedIcon {
        icon_name: String,
        side_px: u16,
        name_variants: NameVariants,
        icon_trim: IconTrim,
        item_theme_path: ItemThemePath,
    },
    IconFile {
        icon_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub enum ResolvedIcon {
    Loaded(LoadedIcon),
    Missing,
    Unloadable(Arc<IconLoadError>),
}

#[derive(Debug, Clone)]
pub struct RequestedIcon {
    pub request: IconRequest,
    pub resolved: ResolvedIcon,
}

#[must_use]
pub fn app_icon_request(app_id: &str) -> IconRequest {
    IconRequest::AppIcon {
        app_id: app_id.to_owned(),
    }
}

#[must_use]
pub fn window_theme_icon_request(icon_name: &str) -> IconRequest {
    IconRequest::NamedIcon {
        icon_name: icon_name.to_owned(),
        side_px: APP_ICON_SIDE_PX,
        name_variants: NameVariants::AsNamed,
        icon_trim: IconTrim::Trimmed,
        item_theme_path: ItemThemePath::Unset,
    }
}

#[must_use]
pub fn notification_image_icon_request(icon_name: &str) -> IconRequest {
    IconRequest::NamedIcon {
        icon_name: icon_name.to_owned(),
        side_px: APP_ICON_SIDE_PX,
        name_variants: NameVariants::AsNamed,
        icon_trim: IconTrim::Trimmed,
        item_theme_path: ItemThemePath::Unset,
    }
}

#[must_use]
pub fn notification_file_icon_request(icon_path: &Path) -> IconRequest {
    IconRequest::IconFile {
        icon_path: icon_path.to_path_buf(),
    }
}

#[must_use]
pub fn menu_row_icon_request(icon_name: &str, side_px: u16) -> IconRequest {
    IconRequest::NamedIcon {
        icon_name: icon_name.to_owned(),
        side_px,
        name_variants: NameVariants::SymbolicFirst,
        icon_trim: IconTrim::DesignSize,
        item_theme_path: ItemThemePath::Unset,
    }
}

#[must_use]
pub fn tray_item_icon_request(icon_name: &str, item_theme_path: &ItemThemePath) -> IconRequest {
    IconRequest::NamedIcon {
        icon_name: icon_name.to_owned(),
        side_px: APP_ICON_SIDE_PX,
        name_variants: NameVariants::SymbolicFirst,
        icon_trim: IconTrim::Trimmed,
        item_theme_path: item_theme_path.clone(),
    }
}

#[must_use]
pub fn window_icon_requests(snapshot: &Snapshot) -> Vec<IconRequest> {
    let distinct_requests: HashSet<IconRequest> = snapshot
        .all_windows()
        .flat_map(WinEntry::icon_candidates)
        .map(|icon_candidate| match icon_candidate {
            IconCandidate::AppId(app_id) => app_icon_request(&app_id),
            IconCandidate::Theme(icon_name) => window_theme_icon_request(&icon_name),
        })
        .collect();
    distinct_requests.into_iter().collect()
}

pub async fn resolved_icons(
    theme_index: Arc<ThemeDirectoryIndex>,
    desktop_entries: Arc<DesktopEntries>,
    requests: Vec<IconRequest>,
) -> Vec<RequestedIcon> {
    stream::iter(requests)
        .map(|request| {
            let theme_index = Arc::clone(&theme_index);
            let desktop_entries = Arc::clone(&desktop_entries);
            async move {
                let resolved = resolved_icon(&theme_index, &desktop_entries, &request).await;
                RequestedIcon { request, resolved }
            }
        })
        .buffer_unordered(ICON_LOADS_IN_FLIGHT)
        .collect()
        .await
}

async fn resolved_icon(
    theme_index: &ThemeDirectoryIndex,
    desktop_entries: &DesktopEntries,
    request: &IconRequest,
) -> ResolvedIcon {
    let located_file = match request {
        IconRequest::AppIcon { app_id } => Ok(app_icon_file(theme_index, desktop_entries, app_id)),
        IconRequest::NamedIcon {
            icon_name,
            side_px,
            name_variants,
            item_theme_path,
            ..
        } => {
            named_icon_file(
                theme_index,
                icon_name,
                *side_px,
                *name_variants,
                item_theme_path,
            )
            .await
        }
        IconRequest::IconFile { icon_path } => Ok(IconFileMatch::Found(icon_path.clone())),
    };
    let icon_trim = match request {
        IconRequest::AppIcon { .. } | IconRequest::IconFile { .. } => IconTrim::Trimmed,
        IconRequest::NamedIcon { icon_trim, .. } => *icon_trim,
    };
    let loaded = match located_file {
        Ok(IconFileMatch::Found(icon_path)) => loaded_icon_file(icon_path, icon_trim).await,
        Ok(IconFileMatch::Missing) => return ResolvedIcon::Missing,
        Err(probe_failure) => Err(probe_failure),
    };
    match loaded {
        Ok(loaded_icon) => ResolvedIcon::Loaded(loaded_icon),
        Err(load_failure) => {
            tracing::warn!(?request, %load_failure, "icon catalog: cannot load an icon");
            ResolvedIcon::Unloadable(Arc::new(load_failure))
        }
    }
}

fn app_icon_file(
    theme_index: &ThemeDirectoryIndex,
    desktop_entries: &DesktopEntries,
    app_id: &str,
) -> IconFileMatch {
    desktop_entries
        .app_icon_candidates(app_id)
        .into_iter()
        .map(|app_icon_candidate| match app_icon_candidate {
            AppIconCandidate::ThemeName(icon_name) => {
                matching_icon_file(theme_index, &icon_name, APP_ICON_SIDE_PX)
            }
            AppIconCandidate::AbsoluteFile(icon_path) => IconFileMatch::Found(icon_path),
        })
        .find(|file_match| matches!(file_match, IconFileMatch::Found(_)))
        .unwrap_or(IconFileMatch::Missing)
}

async fn named_icon_file(
    theme_index: &ThemeDirectoryIndex,
    icon_name: &str,
    side_px: u16,
    name_variants: NameVariants,
    item_theme_path: &ItemThemePath,
) -> Result<IconFileMatch, IconLoadError> {
    for candidate_name in candidate_icon_names(icon_name, name_variants) {
        if let ItemThemePath::Directory(theme_directory) = item_theme_path
            && let IconFileMatch::Found(icon_path) =
                item_theme_path_file(theme_directory, &candidate_name).await?
        {
            return Ok(IconFileMatch::Found(icon_path));
        }
        if let IconFileMatch::Found(icon_path) =
            matching_icon_file(theme_index, &candidate_name, side_px)
        {
            return Ok(IconFileMatch::Found(icon_path));
        }
    }
    Ok(IconFileMatch::Missing)
}

async fn item_theme_path_file(
    theme_directory: &Path,
    icon_name: &str,
) -> Result<IconFileMatch, IconLoadError> {
    for extension in ITEM_THEME_PATH_EXTENSIONS {
        let icon_path = theme_directory.join(format!("{icon_name}.{extension}"));
        match fs::metadata(&icon_path).await {
            Ok(_) => return Ok(IconFileMatch::Found(icon_path)),
            Err(probe_failure) if probe_failure.kind() == io::ErrorKind::NotFound => {}
            Err(read_failure) => {
                return Err(IconLoadError::Unreadable {
                    icon_path,
                    read_failure,
                });
            }
        }
    }
    Ok(IconFileMatch::Missing)
}
