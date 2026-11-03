use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{Task, task::Handle};
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::desktop_entry_app_icons::{self, DesktopEntries},
};

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
    resolve: Option<Handle>,
}

impl DesktopIndex {
    #[must_use]
    pub fn with_app_icons(app_icons: impl IntoIterator<Item = (String, PathBuf)>) -> Self {
        Self {
            app_icons: app_icons.into_iter().collect(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn get(&self) -> Option<&DesktopEntries> {
        self.entries.as_deref()
    }

    #[must_use]
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

    #[must_use]
    pub fn handles_scheme(&self, scheme: &str) -> bool {
        self.get()
            .is_some_and(|entries| entries.handles_scheme(scheme))
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
        let (task, handle) = Task::future(blocking("app icons", move || ResolvedIcons {
            app: desktop_entry_app_icons::resolve_app_icons(&entries, app),
            theme: desktop_entry_app_icons::resolve_theme_icons(&entries, theme),
        }))
        .abortable();
        self.resolve = Some(handle.abort_on_drop());
        task
    }

    pub fn resolve_catalog_icons(&self, app_ids: Vec<String>) -> Task<ResolvedIcons> {
        let Some(entries) = self.entries.clone() else {
            return Task::none();
        };
        Task::future(blocking("app catalog icons", move || ResolvedIcons {
            app: desktop_entry_app_icons::resolve_app_icons(&entries, app_ids),
            theme: HashMap::new(),
        }))
    }

    pub fn merge(&mut self, resolved: ResolvedIcons) {
        self.app_icons.extend(resolved.app);
        self.theme_icons.extend(resolved.theme);
    }
}
