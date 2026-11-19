use std::{cmp::Reverse, collections::HashMap};

use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
    AppCategory, LaunchableApp, OpenAppWindows,
};

use super::launcher_options::OpenAppListing;

pub(super) const MAX_FREQUENT_APPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceWindowPresence {
    Open,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrequentApp {
    pub app: LaunchableApp,
    pub window_presence: WorkspaceWindowPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequentAppScope {
    ThisWorkspace,
    AllWorkspaces,
}

impl FrequentAppScope {
    pub(super) const fn submenu_title(self) -> &'static str {
        match self {
            FrequentAppScope::ThisWorkspace => "Frequently launched on this workspace",
            FrequentAppScope::AllWorkspaces => "Frequently launched",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFrequentApps {
    pub scope: FrequentAppScope,
    pub apps: Vec<FrequentApp>,
}

#[derive(Debug, Clone, Default)]
pub struct AppLaunchUsage {
    pub frequent_by_output: HashMap<String, OutputFrequentApps>,
    pub most_used_app_ids: Vec<String>,
    pub open_windows: HashMap<String, OpenAppWindows>,
}

pub(super) fn apps_shown_in_category(
    catalog: &[LaunchableApp],
    category: AppCategory,
    usage_order: Option<&[String]>,
) -> Vec<LaunchableApp> {
    let mut shown_apps: Vec<LaunchableApp> = catalog
        .iter()
        .filter(|app| app.category == category)
        .cloned()
        .collect();
    if let Some(most_used_app_ids) = usage_order {
        shown_apps.sort_by_cached_key(|app| {
            Reverse(
                most_used_app_ids
                    .iter()
                    .rev()
                    .position(|app_id| *app_id == app.app_id),
            )
        });
    }
    shown_apps
}

pub(super) fn frequent_apps_shown(
    frequent_apps: &[FrequentApp],
    open_app_listing: OpenAppListing,
) -> impl Iterator<Item = &LaunchableApp> {
    frequent_apps
        .iter()
        .filter(move |frequent| {
            !matches!(
                (open_app_listing, frequent.window_presence),
                (OpenAppListing::Hidden, WorkspaceWindowPresence::Open)
            )
        })
        .map(|frequent| &frequent.app)
        .take(MAX_FREQUENT_APPS)
}
