//! The shared `.desktop` index and the app-wide resolved-icon cache: app
//! metadata plus one app-id → icon-path and one theme-name → icon-path map,
//! loaded and resolved off-thread, owned by the application and read by every
//! widget that shows app icons (window list, quick-launch, media sources,
//! notification cards) — rather than each widget keeping a private copy.
//!
//! Callers batch [`DesktopIndex::resolve_missing`] freely on their change
//! triggers: it only spawns work for keys not cached yet, so it is a no-op in
//! the common nothing-changed case. Keys that fail to resolve stay absent and
//! re-try on later calls. The index reloads periodically (a newly installed
//! app or a live theme switch is picked up without a restart), and
//! [`DesktopIndex::set`] clears the cache on reload, which is both its
//! eviction — keys for closed apps drop out — and the bound on that retry
//! churn.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use iced::{Task, task::Handle};

use crate::{
    data_sources::{app_icon, app_icon::DesktopEntries},
    message_subscriptions::blocking,
};

/// How often the index is reloaded.
pub const RELOAD: Duration = Duration::from_secs(300);

/// One off-thread resolve pass's results, merged into the cache on arrival.
#[derive(Debug, Clone, Default)]
pub struct ResolvedIcons {
    app: HashMap<String, PathBuf>,
    theme: HashMap<String, PathBuf>,
}

#[derive(Default)]
pub struct DesktopIndex {
    entries: Option<Arc<DesktopEntries>>,
    app_icons: HashMap<String, PathBuf>,
    theme_icons: HashMap<String, PathBuf>,
    /// Abort-on-drop handle of the in-flight resolve, so a superseded pass
    /// can't land after a newer one.
    resolve: Option<Handle>,
}

impl DesktopIndex {
    pub fn get(&self) -> Option<&DesktopEntries> {
        self.entries.as_deref()
    }

    pub fn arc(&self) -> Option<Arc<DesktopEntries>> {
        self.entries.clone()
    }

    pub fn set(&mut self, entries: Arc<DesktopEntries>) {
        self.entries = Some(entries);
        self.app_icons.clear();
        self.theme_icons.clear();
    }

    pub fn app_icon(&self, app_id: &str) -> Option<&Path> {
        self.app_icons.get(app_id).map(PathBuf::as_path)
    }

    pub fn theme_icon_path(&self, name: &str) -> Option<&Path> {
        self.theme_icons.get(name).map(PathBuf::as_path)
    }

    pub fn resolve_missing(
        &mut self,
        app_keys: Vec<String>,
        theme_keys: Vec<String>,
    ) -> Task<ResolvedIcons> {
        let Some(entries) = self.entries.clone() else {
            return Task::none();
        };
        let app: Vec<String> = app_keys
            .into_iter()
            .filter(|k| !self.app_icons.contains_key(k))
            .collect();
        let theme: Vec<String> = theme_keys
            .into_iter()
            .filter(|k| !self.theme_icons.contains_key(k))
            .collect();
        if app.is_empty() && theme.is_empty() {
            return Task::none();
        }
        let (task, handle) = blocking("app icons", move || ResolvedIcons {
            app: app_icon::resolve_app_icons(&entries, app),
            theme: app_icon::resolve_theme_icons(&entries, theme),
        })
        .abortable();
        self.resolve = Some(handle.abort_on_drop());
        task
    }

    pub fn merge(&mut self, resolved: ResolvedIcons) {
        self.app_icons.extend(resolved.app);
        self.theme_icons.extend(resolved.theme);
    }
}

pub fn load() -> Task<Arc<DesktopEntries>> {
    blocking("desktop entries", DesktopEntries::load).map(Arc::new)
}
