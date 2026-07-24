//! The shared `.desktop` index: app metadata and icon resolution, loaded once
//! off-thread and reloaded periodically (so a newly installed app or a live
//! theme switch is picked up without a restart). Owned by the application and
//! read by every widget that resolves app icons — the window list and the
//! notification cards — rather than being any one widget's private state.

use std::{sync::Arc, time::Duration};

use iced::Task;

use crate::{data_sources::app_icon::DesktopEntries, message_subscriptions::blocking};

/// How often the index is reloaded.
pub const RELOAD: Duration = Duration::from_secs(300);

#[derive(Default)]
pub struct DesktopIndex {
    entries: Option<Arc<DesktopEntries>>,
}

impl DesktopIndex {
    /// The loaded index, or `None` before the first load completes.
    pub fn get(&self) -> Option<&DesktopEntries> {
        self.entries.as_deref()
    }

    /// A cheap `Arc` clone of the index, to move into an off-thread resolve.
    pub fn arc(&self) -> Option<Arc<DesktopEntries>> {
        self.entries.clone()
    }

    pub fn set(&mut self, entries: Arc<DesktopEntries>) {
        self.entries = Some(entries);
    }
}

/// Load (or reload) the index off-thread.
pub fn load() -> Task<Arc<DesktopEntries>> {
    blocking("desktop entries", DesktopEntries::load).map(Arc::new)
}
