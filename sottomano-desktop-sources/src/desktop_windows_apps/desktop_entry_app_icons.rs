use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use freedesktop_icons::lookup;

use super::niri_window_snapshot;
use crate::desktop_windows_apps::{
    desktop_entry_app_catalog::LaunchableApp, window_title_labels::IconCandidate,
};

#[derive(Debug)]
pub struct DesktopEntries {
    entries: Vec<DesktopEntry>,
    theme: Option<String>,
}

impl Default for DesktopEntries {
    fn default() -> Self {
        Self::empty()
    }
}

impl DesktopEntries {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            theme: freedesktop_icons::default_theme_gtk(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_entries(entries: Vec<DesktopEntry>) -> Self {
        Self {
            entries,
            theme: None,
        }
    }

    #[must_use]
    pub fn load() -> Self {
        let locales: Vec<String> = Vec::new();
        let entries = Iter::new(default_paths())
            .filter_map(|p| DesktopEntry::from_path(p, Some(&locales)).ok())
            .collect();
        Self {
            entries,
            theme: freedesktop_icons::default_theme_gtk(),
        }
    }

    #[must_use]
    pub fn icon_for(&self, app_id: &str) -> Option<PathBuf> {
        if app_id.is_empty() {
            return None;
        }
        self.resolve_icon(app_id)
    }

    fn resolve_icon(&self, app_id: &str) -> Option<PathBuf> {
        if let Some(entry) = self.find(app_id)
            && let Some(icon_name) = entry.icon()
            && let Some(path) = self.theme_lookup(icon_name)
        {
            return Some(path);
        }
        let lower = app_id.to_lowercase();
        let dotted_suffix = lower
            .rsplit('.')
            .next()
            .filter(|s| *s != lower)
            .map(str::to_string);
        [Some(app_id.to_string()), Some(lower), dotted_suffix]
            .into_iter()
            .flatten()
            .find_map(|name| self.theme_lookup(&name))
            .or_else(|| self.theme_lookup("application-x-executable"))
    }

    fn find(&self, app_id: &str) -> Option<&DesktopEntry> {
        use freedesktop_desktop_entry::unicase::Ascii;
        freedesktop_desktop_entry::find_app_by_id(&self.entries, Ascii::new(app_id))
    }

    #[must_use]
    pub fn launchable_app(&self, app_id: &str) -> Option<LaunchableApp> {
        LaunchableApp::from_shown_entry(
            self.find(app_id)?,
            &freedesktop_desktop_entry::current_desktop().unwrap_or_default(),
        )
    }

    #[must_use]
    pub fn launchable_apps(&self) -> Vec<LaunchableApp> {
        let current_desktops = freedesktop_desktop_entry::current_desktop().unwrap_or_default();
        let mut listed_app_ids = HashSet::new();
        let mut launchable_apps: Vec<LaunchableApp> = self
            .entries
            .iter()
            .filter(|entry| listed_app_ids.insert(entry.id()))
            .filter_map(|entry| LaunchableApp::from_shown_entry(entry, &current_desktops))
            .collect();
        launchable_apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.app_id.clone()));
        launchable_apps
    }

    #[must_use]
    pub fn handles_scheme(&self, scheme: &str) -> bool {
        let handler = format!("x-scheme-handler/{scheme}");
        self.entries
            .iter()
            .filter(|entry| !entry.hidden())
            .any(|entry| {
                entry
                    .mime_type()
                    .is_some_and(|mime_types| mime_types.contains(&handler.as_str()))
            })
    }

    #[must_use]
    pub fn theme_icon(&self, name: &str) -> Option<PathBuf> {
        self.theme_lookup(name)
    }

    fn theme_lookup(&self, name: &str) -> Option<PathBuf> {
        let mut query = lookup(name).with_size(48).with_cache();
        if let Some(theme) = &self.theme {
            query = query.with_theme(theme);
        }
        query.find()
    }
}

#[must_use]
pub fn app_icon_keys(snapshot: &niri_window_snapshot::Snapshot) -> Vec<String> {
    icon_keys(snapshot, |c| match c {
        IconCandidate::AppId(id) => Some(id),
        IconCandidate::Theme(_) => None,
    })
}

#[must_use]
pub fn theme_icon_keys(snapshot: &niri_window_snapshot::Snapshot) -> Vec<String> {
    icon_keys(snapshot, |c| match c {
        IconCandidate::Theme(name) => Some(name),
        IconCandidate::AppId(_) => None,
    })
}

fn icon_keys(
    snapshot: &niri_window_snapshot::Snapshot,
    pick: impl Fn(IconCandidate) -> Option<String>,
) -> Vec<String> {
    let mut keys: Vec<String> = snapshot
        .all_windows()
        .flat_map(|w| w.icon_candidates().into_iter().filter_map(&pick))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[must_use]
pub fn resolve_app_icons(entries: &DesktopEntries, ids: Vec<String>) -> HashMap<String, PathBuf> {
    ids.into_iter()
        .filter_map(|id| {
            entries
                .icon_for(&id)
                .map(|path| (id, super::icon_whitespace_trim::trimmed(&path)))
        })
        .collect()
}

#[must_use]
pub fn resolve_theme_icons(
    entries: &DesktopEntries,
    names: Vec<String>,
) -> HashMap<String, PathBuf> {
    names
        .into_iter()
        .filter_map(|name| {
            entries
                .theme_icon(&name)
                .map(|path| (name, super::icon_whitespace_trim::trimmed(&path)))
        })
        .collect()
}
