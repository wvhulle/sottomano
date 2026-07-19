//! App icon resolution via `.desktop` entries and the icon theme.

use std::{collections::HashMap, path::PathBuf, sync::RwLock};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use freedesktop_icons::lookup;

#[derive(Debug)]
pub struct DesktopEntries {
    entries: Vec<DesktopEntry>,
    theme: Option<String>,
    icon_cache: RwLock<HashMap<String, Option<PathBuf>>>,
}

impl DesktopEntries {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            theme: freedesktop_icons::default_theme_gtk(),
            icon_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn load() -> Self {
        let locales: Vec<String> = Vec::new();
        let entries = Iter::new(default_paths())
            .filter_map(|p| DesktopEntry::from_path(p, Some(&locales)).ok())
            .collect();
        Self {
            entries,
            theme: freedesktop_icons::default_theme_gtk(),
            icon_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn icon_for(&self, app_id: &str) -> Option<PathBuf> {
        if app_id.is_empty() {
            return None;
        }
        if let Some(cached) = self.icon_cache.read().unwrap().get(app_id) {
            return cached.clone();
        }
        let resolved = self.resolve_icon(app_id);
        self.icon_cache
            .write()
            .unwrap()
            .insert(app_id.to_string(), resolved.clone());
        resolved
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

    fn theme_lookup(&self, name: &str) -> Option<PathBuf> {
        let mut query = lookup(name).with_size(48).with_cache();
        if let Some(theme) = &self.theme {
            query = query.with_theme(theme);
        }
        query.find()
    }
}
