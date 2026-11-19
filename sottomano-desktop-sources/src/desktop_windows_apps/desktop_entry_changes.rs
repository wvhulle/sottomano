use std::sync::Arc;

use freedesktop_desktop_entry::default_paths;
use futures::{Stream, StreamExt};

use crate::{
    blocking_background_work::blocking,
    desktop_windows_apps::desktop_entries::DesktopEntries,
    system_protocol_clients::inotify_directory_changes::{WatchTarget, changes},
    theme_icons::theme_directory_index::{IconSearchRoots, xdg_icon_search_roots},
};

const CHANGE_BURST_LIMIT: usize = 64;

pub async fn load() -> Arc<DesktopEntries> {
    Arc::new(blocking("desktop entries", DesktopEntries::load).await)
}

pub fn catalog_file_changes() -> impl Stream<Item = ()> + Send {
    let IconSearchRoots {
        theme_roots,
        pixmap_directories,
    } = xdg_icon_search_roots();
    changes(
        default_paths()
            .chain(theme_roots)
            .chain(pixmap_directories)
            .map(WatchTarget::EntriesOf)
            .collect(),
    )
    .ready_chunks(CHANGE_BURST_LIMIT)
    .map(drop)
}
