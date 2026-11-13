use std::{collections::HashMap, sync::Arc};

use iced::{Task, task::Handle};
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{
        desktop_entry_app_icons::{self, DesktopEntries, SchemeHandling},
        icon_whitespace_trim::TrimmedIcon,
    },
};

use crate::widgets::icon_handle::IconHandle;

#[derive(Debug, Clone, Default)]
pub struct ResolvedIcons {
    app: HashMap<String, IconHandle>,
    theme: HashMap<String, IconHandle>,
}

#[derive(Default)]
pub struct DesktopIndex {
    entries: Option<Arc<DesktopEntries>>,
    app_icons: HashMap<String, IconHandle>,
    theme_icons: HashMap<String, IconHandle>,
    resolve: Option<Handle>,
}

impl DesktopIndex {
    #[must_use]
    pub fn with_app_icons(app_icons: impl IntoIterator<Item = (String, IconHandle)>) -> Self {
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

    #[must_use]
    pub fn app_icon(&self, app_id: &str) -> Option<&IconHandle> {
        self.app_icons.get(app_id)
    }

    #[must_use]
    pub fn scheme_handling(&self, scheme: &str) -> SchemeHandling {
        self.get().map_or(SchemeHandling::Unhandled, |entries| {
            entries.scheme_handling(scheme)
        })
    }

    #[must_use]
    pub fn theme_icon(&self, name: &str) -> Option<&IconHandle> {
        self.theme_icons.get(name)
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
            app: icon_handles_by_key(desktop_entry_app_icons::resolve_app_icons(&entries, app)),
            theme: icon_handles_by_key(desktop_entry_app_icons::resolve_theme_icons(
                &entries, theme,
            )),
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
            app: icon_handles_by_key(desktop_entry_app_icons::resolve_app_icons(
                &entries, app_ids,
            )),
            theme: HashMap::new(),
        }))
    }

    pub fn merge(&mut self, resolved: ResolvedIcons) {
        self.app_icons.extend(resolved.app);
        self.theme_icons.extend(resolved.theme);
    }
}

fn icon_handles_by_key(trimmed_icons: HashMap<String, TrimmedIcon>) -> HashMap<String, IconHandle> {
    trimmed_icons
        .into_iter()
        .map(|(icon_key, trimmed_icon)| (icon_key, IconHandle::from(&trimmed_icon)))
        .collect()
}
