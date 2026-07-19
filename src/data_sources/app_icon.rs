//! App icon resolution, ported from odin's `WindowIcon._resolveAppId`: a
//! window's `app_id` → an icon file on disk, via the same three-step fallback
//! odin uses —
//! 1. the matching `.desktop` entry's `Icon=` field, resolved through the icon
//!    theme;
//! 2. theme-name lookups of the app_id itself (as-is, lowercased, last dotted
//!    segment — `org.kde.dolphin` → `dolphin`);
//! 3. a generic fallback icon, so something always renders.
//!
//! `freedesktop-desktop-entry` supplies the `.desktop` index;
//! `freedesktop-icons` walks the on-disk icon theme (respecting the user's
//! configured theme via `default_theme_gtk`).

use std::{collections::HashMap, path::PathBuf, sync::RwLock};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use freedesktop_icons::lookup;

/// A snapshot of installed `.desktop` entries, read once at startup (odin's
/// `DesktopEntries` singleton). Re-scanning per lookup would mean a directory
/// walk on every window-list render; app installs changing mid-session is rare
/// enough that a process-lifetime snapshot is the right tradeoff.
#[derive(Debug)]
pub struct DesktopEntries {
    entries: Vec<DesktopEntry>,
    theme: Option<String>,
    /// Memoised `app_id` → icon-path results. `icon_for` is called for every
    /// window on every frame from `view()`, but resolution (a `.desktop`
    /// scan plus an on-disk icon-theme walk via `freedesktop_icons`) costs
    /// ~10 ms per app — turning one window-list render into ~130 ms and
    /// stalling the whole bar surface. The mapping is stable for a given
    /// app_id, so we resolve once and reuse; `view()` then only
    /// does a cheap map lookup, keeping it free of I/O (the iced rule).
    /// Interior mutability because `view()` takes `&self`, and a `RwLock`
    /// (not `RefCell`) because the bar state must be `Sync`; contention is
    /// nil (reads dominate, on one thread).
    icon_cache: RwLock<HashMap<String, Option<PathBuf>>>,
}

impl DesktopEntries {
    /// An empty index — used only if the startup scan task itself panics, so
    /// the window list still renders (icons just fall back to the generic
    /// executable icon).
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

    /// Resolve a window's `app_id` to an icon file (memoised — see
    /// `icon_cache`). Cheap on the hot `view()` path; the expensive lookup
    /// happens once per app_id.
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

    /// The actual fallback chain, odin's `WindowIcon._resolveAppId`. Called
    /// once per app_id by `icon_for`, which memoises the result.
    fn resolve_icon(&self, app_id: &str) -> Option<PathBuf> {
        if let Some(entry) = self.find(app_id)
            && let Some(icon_name) = entry.icon()
            && let Some(path) = self.theme_lookup(icon_name)
        {
            return Some(path);
        }
        // Theme-name variants of the app_id itself: as-is, lowercased, last dotted
        // segment lowercased (`org.kde.dolphin` → `dolphin`) — odin's exact tries list.
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
