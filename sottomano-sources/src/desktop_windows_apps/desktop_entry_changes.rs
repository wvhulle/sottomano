use std::sync::Arc;

use freedesktop_desktop_entry::default_paths;
use futures::{Stream, StreamExt};

use crate::{
    blocking_background_work::blocking,
    desktop_windows_apps::desktop_entry_app_icons::DesktopEntries,
    system_protocol_clients::inotify_directory_changes::{WatchTarget, changes},
};

const CHANGE_BURST_LIMIT: usize = 64;

pub async fn load() -> Arc<DesktopEntries> {
    Arc::new(blocking("desktop entries", DesktopEntries::load).await)
}

pub fn application_changes() -> impl Stream<Item = ()> + Send {
    changes(default_paths().map(WatchTarget::EntriesOf).collect())
        .ready_chunks(CHANGE_BURST_LIMIT)
        .map(drop)
}
