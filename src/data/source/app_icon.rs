//! App icon resolution via `.desktop` entries and the icon theme.
//!
//! Resolution stays off the render path: one step gathers the distinct
//! app-ids/theme-names currently needed by open windows, and a second resolves
//! a batch of them off-thread. App-ids and theme-names stay separate pipelines
//! — they resolve differently and miss differently — so no cache lives here;
//! the caller's maps are the only cache.

use std::{collections::HashMap, path::PathBuf};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use freedesktop_icons::lookup;
use niri_ipc::state::EventStreamState;

use super::{app_rules::IconCandidate, niri};

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

    /// Whether an app-id is one a normal launcher would show: it has a desktop
    /// entry that is neither `NoDisplay` nor `Hidden`. Excludes a shell's own
    /// hidden entry and wrapped binaries with no entry, so quick-launch only
    /// ranks apps whose icon it can actually show and launch (odin's
    /// `launchable`).
    pub fn is_launchable(&self, app_id: &str) -> bool {
        self.find(app_id)
            .is_some_and(|entry| !entry.no_display() && !entry.hidden())
    }

    /// The command to spawn for an app-id, parsed from its desktop entry's
    /// `Exec` by the crate's spec-compliant parser (quoting honoured, field
    /// codes like `%U`/`%f` resolved against no files) — enough to launch it
    /// directly (odin's `DesktopEntries.byId(id).execute()`). `None` when there
    /// is no entry or its `Exec` is missing/empty/malformed.
    pub fn exec_command(&self, app_id: &str) -> Option<Vec<String>> {
        self.find(app_id)?
            .parse_exec()
            .ok()
            .filter(|c| !c.is_empty())
    }

    /// Resolve an icon-theme name directly, bypassing the `.desktop`/app-id
    /// lookup — for icons picked by convention rather than by app-id, e.g. a
    /// terminal's command-word icon.
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

/// Distinct app-ids across every window niri currently knows about. Every
/// window carries an app-id fallback candidate, so this covers all windows,
/// not just ones missing a more specific icon.
pub fn app_icon_keys(niri_state: &EventStreamState) -> Vec<String> {
    icon_keys(niri_state, |c| match c {
        IconCandidate::AppId(id) => Some(id),
        _ => None,
    })
}

/// Distinct icon-theme names across every window (e.g. a terminal's
/// command-word icon) — only windows whose formatting rule picks a theme-name
/// candidate contribute one.
pub fn theme_icon_keys(niri_state: &EventStreamState) -> Vec<String> {
    icon_keys(niri_state, |c| match c {
        IconCandidate::Theme(name) => Some(name),
        _ => None,
    })
}

fn icon_keys(
    niri_state: &EventStreamState,
    pick: impl Fn(IconCandidate) -> Option<String>,
) -> Vec<String> {
    let mut keys: Vec<String> = niri::all_windows(niri_state)
        .iter()
        .flat_map(|w| {
            super::app_rules::rule_for(&w.app_id)
                .icon_candidates(w)
                .into_iter()
                .filter_map(&pick)
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Resolve a batch of `.desktop`-entry app-ids. Unresolvable ones are simply
/// absent from the result.
pub fn resolve_app_icons(entries: &DesktopEntries, ids: Vec<String>) -> HashMap<String, PathBuf> {
    ids.into_iter()
        .filter_map(|id| {
            entries
                .icon_for(&id)
                .map(|path| (id, super::icon_trim::trimmed(&path)))
        })
        .collect()
}

/// Resolve a batch of icon-theme names. Unresolvable ones are simply absent.
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
