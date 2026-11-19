use std::{collections::HashSet, path::PathBuf};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};

use crate::desktop_windows_apps::desktop_entry_app_catalog::LaunchableApp;

const UNIDENTIFIED_APP_ICON: &str = "application-x-executable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIconCandidate {
    ThemeName(String),
    AbsoluteFile(PathBuf),
}

#[derive(Debug, Default)]
pub struct DesktopEntries {
    entries: Vec<DesktopEntry>,
}
impl DesktopEntries {
    #[cfg(test)]
    pub(super) fn with_entries(entries: Vec<DesktopEntry>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn load() -> Self {
        let locales: Vec<String> = Vec::new();
        let entries = Iter::new(default_paths())
            .filter_map(|entry_path| {
                DesktopEntry::from_path(&entry_path, Some(&locales))
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "desktop entries: cannot parse {}: {parse_failure}",
                            entry_path.display()
                        );
                    })
                    .ok()
            })
            .collect();
        Self { entries }
    }

    #[must_use]
    pub fn app_icon_candidates(&self, app_id: &str) -> Vec<AppIconCandidate> {
        let declared_icon = self
            .find(app_id)
            .and_then(DesktopEntry::icon)
            .map(|declared_icon| {
                let declared_path = PathBuf::from(declared_icon);
                if declared_path.is_absolute() {
                    AppIconCandidate::AbsoluteFile(declared_path)
                } else {
                    AppIconCandidate::ThemeName(declared_icon.to_owned())
                }
            });
        let lowercase_id = app_id.to_lowercase();
        let last_dotted_segment = lowercase_id
            .rsplit('.')
            .next()
            .filter(|segment| *segment != lowercase_id)
            .map(str::to_owned);
        declared_icon
            .into_iter()
            .chain(
                [
                    Some(app_id.to_owned()),
                    Some(lowercase_id.clone()),
                    last_dotted_segment,
                ]
                .into_iter()
                .flatten()
                .filter(|candidate_name| !candidate_name.is_empty())
                .map(AppIconCandidate::ThemeName),
            )
            .chain([AppIconCandidate::ThemeName(
                UNIDENTIFIED_APP_ICON.to_owned(),
            )])
            .collect()
    }

    fn find(&self, app_id: &str) -> Option<&DesktopEntry> {
        use freedesktop_desktop_entry::unicase::Ascii;
        freedesktop_desktop_entry::find_app_by_id(&self.entries, Ascii::new(app_id))
    }

    #[must_use]
    pub fn launchable_app(&self, app_id: &str) -> Option<LaunchableApp> {
        LaunchableApp::shown_on_desktops(
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
            .filter_map(|entry| LaunchableApp::shown_on_desktops(entry, &current_desktops))
            .collect();
        launchable_apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.app_id.clone()));
        launchable_apps
    }

    #[must_use]
    pub fn scheme_handling(&self, scheme: &str) -> SchemeHandling {
        let handler = format!("x-scheme-handler/{scheme}");
        if self
            .entries
            .iter()
            .filter(|entry| !entry.hidden())
            .any(|entry| {
                entry
                    .mime_type()
                    .is_some_and(|mime_types| mime_types.contains(&handler.as_str()))
            })
        {
            SchemeHandling::Handled
        } else {
            SchemeHandling::Unhandled
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeHandling {
    Handled,
    Unhandled,
}
