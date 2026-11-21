use std::{
    cmp::Reverse,
    fs, io,
    path::{Path, PathBuf},
};

use super::{
    keyvalues_text_parser::{KeyValuesBlock, KeyValuesSyntaxError, parse_keyvalues_document},
    steam_log_lines::SteamAppId,
    steam_root_location::SteamRoot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSteamApp {
    pub app_id: SteamAppId,
    pub name: String,
    pub install_directory: PathBuf,
    pub size_on_disk_bytes: u64,
    pub bytes_to_download: u64,
    pub bytes_downloaded: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamLibraryFolder {
    pub folder_path: PathBuf,
    pub installed_apps: Vec<InstalledSteamApp>,
}

#[derive(Debug, thiserror::Error)]
pub enum SteamManifestFailure {
    #[error("Steam manifest {path} unreadable: {read_failure}")]
    Read {
        path: PathBuf,
        read_failure: io::Error,
    },
    #[error("Steam manifest {path}: {syntax_failure}")]
    Syntax {
        path: PathBuf,
        syntax_failure: KeyValuesSyntaxError,
    },
    #[error("Steam manifest {path} lacks its {missing_key} entry")]
    MissingEntry {
        path: PathBuf,
        missing_key: &'static str,
    },
}

fn keyvalues_file(path: &Path) -> Result<KeyValuesBlock, SteamManifestFailure> {
    let text = fs::read_to_string(path).map_err(|read_failure| SteamManifestFailure::Read {
        path: path.to_owned(),
        read_failure,
    })?;
    parse_keyvalues_document(&text).map_err(|syntax_failure| SteamManifestFailure::Syntax {
        path: path.to_owned(),
        syntax_failure,
    })
}

fn required_block<'document>(
    document: &'document KeyValuesBlock,
    path: &Path,
    key: &'static str,
) -> Result<&'document KeyValuesBlock, SteamManifestFailure> {
    document
        .block(key)
        .ok_or_else(|| SteamManifestFailure::MissingEntry {
            path: path.to_owned(),
            missing_key: key,
        })
}

fn installed_app(
    library_folder: &Path,
    manifest_path: &Path,
) -> Result<InstalledSteamApp, SteamManifestFailure> {
    let document = keyvalues_file(manifest_path)?;
    let app_state = required_block(&document, manifest_path, "AppState")?;
    let missing = |missing_key| SteamManifestFailure::MissingEntry {
        path: manifest_path.to_owned(),
        missing_key,
    };
    Ok(InstalledSteamApp {
        app_id: SteamAppId {
            number: app_state.parsed("appid").ok_or_else(|| missing("appid"))?,
        },
        name: app_state
            .text("name")
            .ok_or_else(|| missing("name"))?
            .to_owned(),
        install_directory: library_folder.join("steamapps").join("common").join(
            app_state
                .text("installdir")
                .ok_or_else(|| missing("installdir"))?,
        ),
        size_on_disk_bytes: app_state.parsed("SizeOnDisk").unwrap_or_default(),
        bytes_to_download: app_state.parsed("BytesToDownload").unwrap_or_default(),
        bytes_downloaded: app_state.parsed("BytesDownloaded").unwrap_or_default(),
    })
}

fn manifest_path(library_folder: &Path, app_id: SteamAppId) -> PathBuf {
    library_folder
        .join("steamapps")
        .join(format!("appmanifest_{}.acf", app_id.number))
}

pub fn library_folder_paths(steam_root: &SteamRoot) -> Result<Vec<PathBuf>, SteamManifestFailure> {
    let library_file = steam_root.library_folders_file();
    let document = keyvalues_file(&library_file)?;
    let folders = required_block(&document, &library_file, "libraryfolders")?;
    Ok(folders
        .blocks()
        .filter_map(|(_, folder)| folder.text("path").map(PathBuf::from))
        .collect())
}

#[must_use]
pub fn installed_app_in_libraries(
    library_folders: &[PathBuf],
    app_id: SteamAppId,
) -> Option<InstalledSteamApp> {
    library_folders.iter().find_map(|library_folder| {
        let manifest_path = manifest_path(library_folder, app_id);
        manifest_path
            .exists()
            .then(|| installed_app(library_folder, &manifest_path))?
            .inspect_err(|manifest_failure| tracing::debug!("{manifest_failure}"))
            .ok()
    })
}

#[must_use]
pub fn library_folder_contents(library_folder: &Path) -> SteamLibraryFolder {
    let manifests_directory = library_folder.join("steamapps");
    let mut installed_apps: Vec<InstalledSteamApp> = fs::read_dir(&manifests_directory)
        .inspect_err(|read_failure| {
            tracing::debug!(
                "Steam library {} unreadable: {read_failure}",
                manifests_directory.display()
            );
        })
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("appmanifest_"))
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("acf"))
        })
        .filter_map(|manifest_path| {
            installed_app(library_folder, &manifest_path)
                .inspect_err(|manifest_failure| tracing::debug!("{manifest_failure}"))
                .ok()
        })
        .collect();
    installed_apps.sort_by_key(|app| Reverse(app.size_on_disk_bytes));
    SteamLibraryFolder {
        folder_path: library_folder.to_owned(),
        installed_apps,
    }
}
