use std::{collections::HashMap, path::PathBuf};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use freedesktop_icons::lookup;

use super::niri;
use crate::data::source::app_rules::IconCandidate;

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
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            theme: freedesktop_icons::default_theme_gtk(),
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
        }
    }

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

    pub fn is_launchable(&self, app_id: &str) -> bool {
        self.find(app_id)
            .is_some_and(|entry| !entry.no_display() && !entry.hidden())
    }

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

    pub fn exec_command(&self, app_id: &str) -> Option<Vec<String>> {
        self.find(app_id)?
            .parse_exec()
            .ok()
            .filter(|c| !c.is_empty())
    }

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

pub fn app_icon_keys(snapshot: &niri::Snapshot) -> Vec<String> {
    icon_keys(snapshot, |c| match c {
        IconCandidate::AppId(id) => Some(id),
        IconCandidate::Theme(_) => None,
    })
}

pub fn theme_icon_keys(snapshot: &niri::Snapshot) -> Vec<String> {
    icon_keys(snapshot, |c| match c {
        IconCandidate::Theme(name) => Some(name),
        IconCandidate::AppId(_) => None,
    })
}

fn icon_keys(
    snapshot: &niri::Snapshot,
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

pub fn resolve_app_icons(entries: &DesktopEntries, ids: Vec<String>) -> HashMap<String, PathBuf> {
    ids.into_iter()
        .filter_map(|id| {
            entries
                .icon_for(&id)
                .map(|path| (id, super::icon_trim::trimmed(&path)))
        })
        .collect()
}

pub fn resolve_theme_icons(
    entries: &DesktopEntries,
    names: Vec<String>,
) -> HashMap<String, PathBuf> {
    names
        .into_iter()
        .filter_map(|name| {
            entries
                .theme_icon(&name)
                .map(|path| (name, super::icon_trim::trimmed(&path)))
        })
        .collect()
}
