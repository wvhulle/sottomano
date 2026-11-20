use std::{future::ready, io};

use futures::{Stream, StreamExt, stream};
use rustix::fs::statvfs;

use super::{
    steam_app_manifests::{
        SteamLibraryFolder, SteamManifestFailure, library_folder_contents, library_folder_paths,
    },
    steam_root_location::SteamRoot,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeTriggerStreams, ChangeWatchFailure, SnapshotDeliveryStreams},
    system_protocol_clients::inotify_directory_changes::{self, WatchTarget},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamLibraryFolderUsage {
    pub library_folder: SteamLibraryFolder,
    pub free_bytes: u64,
}

impl SteamLibraryFolderUsage {
    #[must_use]
    pub fn installed_bytes(&self) -> u64 {
        self.library_folder
            .installed_apps
            .iter()
            .map(|app| app.size_on_disk_bytes)
            .sum()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamLibraryUsage {
    pub folders: Vec<SteamLibraryFolderUsage>,
}

#[derive(Debug, thiserror::Error)]
pub enum SteamLibraryReadFailure {
    #[error(transparent)]
    Watch(#[from] ChangeWatchFailure),
    #[error(transparent)]
    Manifest(#[from] SteamManifestFailure),
    #[error("free space of Steam library unreadable: {0}")]
    FreeSpace(#[from] io::Error),
    #[error("free space of Steam library exceeds 64 bits")]
    FreeSpaceOverflow,
}

fn read_library_usage(steam_root: &SteamRoot) -> Result<SteamLibraryUsage, SteamLibraryReadFailure> {
    let folders = library_folder_paths(steam_root)?
        .iter()
        .map(|folder_path| {
            let filesystem = statvfs(folder_path).map_err(io::Error::from)?;
            Ok(SteamLibraryFolderUsage {
                library_folder: library_folder_contents(folder_path),
                free_bytes: filesystem
                    .f_bavail
                    .checked_mul(filesystem.f_frsize)
                    .ok_or(SteamLibraryReadFailure::FreeSpaceOverflow)?,
            })
        })
        .collect::<Result<Vec<_>, SteamLibraryReadFailure>>()?;
    Ok(SteamLibraryUsage { folders })
}

impl SnapshotSource for SteamLibraryUsage {
    type Error = SteamLibraryReadFailure;
    type StreamInputs = ();

    const NAME: &'static str = "Steam library";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let watched = SteamRoot::located().map(|steam_root| {
            let library_watches = library_folder_paths(&steam_root)
                .unwrap_or_else(|manifest_failure| {
                    tracing::debug!("{manifest_failure}");
                    Vec::new()
                })
                .into_iter()
                .map(|library_folder| WatchTarget::EntriesOf(library_folder.join("steamapps")))
                .chain([WatchTarget::File(steam_root.library_folders_file())])
                .collect();
            inotify_directory_changes::changes(library_watches)
                .triggered_rereads(move || ready(read_library_usage(&steam_root)))
        });
        stream::iter(watched)
            .flatten()
            .map(SnapshotDelivery::live_push)
            .repeats_as_unchanged()
    }
}
