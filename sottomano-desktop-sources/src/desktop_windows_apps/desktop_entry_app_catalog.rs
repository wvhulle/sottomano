use std::{borrow::Cow, cmp::Reverse, collections::HashMap};

use freedesktop_desktop_entry::DesktopEntry;

use super::{compositor_window_state::CompositorWindow, desktop_entry_app_icons::DesktopEntries};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppCategory {
    Development,
    Education,
    Game,
    Graphics,
    Multimedia,
    Network,
    Office,
    Science,
    Settings,
    System,
    Utility,
    Other,
}

impl AppCategory {
    pub const ALL: [AppCategory; 12] = [
        AppCategory::Development,
        AppCategory::Education,
        AppCategory::Game,
        AppCategory::Graphics,
        AppCategory::Multimedia,
        AppCategory::Network,
        AppCategory::Office,
        AppCategory::Science,
        AppCategory::Settings,
        AppCategory::System,
        AppCategory::Utility,
        AppCategory::Other,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            AppCategory::Development => "Development",
            AppCategory::Education => "Education",
            AppCategory::Game => "Games",
            AppCategory::Graphics => "Graphics",
            AppCategory::Multimedia => "Multimedia",
            AppCategory::Network => "Internet",
            AppCategory::Office => "Office",
            AppCategory::Science => "Science",
            AppCategory::Settings => "Settings",
            AppCategory::System => "System",
            AppCategory::Utility => "Utilities",
            AppCategory::Other => "Other",
        }
    }

    #[must_use]
    pub const fn stored_name(self) -> &'static str {
        match self {
            AppCategory::Development => "development",
            AppCategory::Education => "education",
            AppCategory::Game => "game",
            AppCategory::Graphics => "graphics",
            AppCategory::Multimedia => "multimedia",
            AppCategory::Network => "network",
            AppCategory::Office => "office",
            AppCategory::Science => "science",
            AppCategory::Settings => "settings",
            AppCategory::System => "system",
            AppCategory::Utility => "utility",
            AppCategory::Other => "other",
        }
    }

    fn from_registered_main_category(declared: &str) -> Option<Self> {
        match declared {
            "AudioVideo" | "Audio" | "Video" => Some(AppCategory::Multimedia),
            "Development" => Some(AppCategory::Development),
            "Education" => Some(AppCategory::Education),
            "Game" => Some(AppCategory::Game),
            "Graphics" => Some(AppCategory::Graphics),
            "Network" => Some(AppCategory::Network),
            "Office" => Some(AppCategory::Office),
            "Science" => Some(AppCategory::Science),
            "Settings" => Some(AppCategory::Settings),
            "System" => Some(AppCategory::System),
            "Utility" => Some(AppCategory::Utility),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchableApp {
    pub app_id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: AppCategory,
    pub launch_command: Vec<String>,
    pub runs_in_terminal: bool,
}

impl LaunchableApp {
    pub(super) fn from_shown_entry(
        entry: &DesktopEntry,
        current_desktops: &[String],
    ) -> Option<Self> {
        if !is_shown_on_desktops(entry, current_desktops) {
            return None;
        }
        let no_locales: &[&str] = &[];
        Some(Self {
            app_id: entry.id().to_owned(),
            name: entry
                .name(no_locales)
                .map_or_else(|| entry.id().to_owned(), Cow::into_owned),
            description: entry.comment(no_locales).map(Cow::into_owned),
            category: entry
                .categories()
                .unwrap_or_default()
                .into_iter()
                .find_map(AppCategory::from_registered_main_category)
                .unwrap_or(AppCategory::Other),
            launch_command: entry
                .parse_exec()
                .ok()
                .filter(|command| !command.is_empty())?,
            runs_in_terminal: entry.terminal(),
        })
    }
}

fn is_shown_on_desktops(entry: &DesktopEntry, current_desktops: &[String]) -> bool {
    let lists_current_desktop = |listed_desktops: Vec<&str>| {
        listed_desktops.iter().any(|listed_desktop| {
            current_desktops
                .iter()
                .any(|current_desktop| current_desktop.eq_ignore_ascii_case(listed_desktop))
        })
    };
    !entry.no_display()
        && !entry.hidden()
        && entry
            .type_()
            .is_none_or(|entry_kind| entry_kind == "Application")
        && entry.only_show_in().is_none_or(lists_current_desktop)
        && !entry.not_show_in().is_some_and(lists_current_desktop)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAppWindows {
    pub latest_focused_window: u64,
    pub window_count: usize,
}

#[must_use]
pub fn open_windows_by_app<'window>(
    windows: impl IntoIterator<Item = &'window CompositorWindow>,
    entries: &DesktopEntries,
) -> HashMap<String, OpenAppWindows> {
    let mut windows_by_app: HashMap<String, Vec<&CompositorWindow>> = HashMap::new();
    for window in windows {
        let launchable_app = window
            .app_id
            .as_deref()
            .and_then(|app_id| entries.launchable_app(app_id));
        if let Some(app) = launchable_app {
            windows_by_app.entry(app.app_id).or_default().push(window);
        }
    }
    windows_by_app
        .into_iter()
        .filter_map(|(app_id, app_windows)| {
            let latest_window = app_windows
                .iter()
                .max_by_key(|window| (window.focused_at_monotonic, Reverse(window.id)))?;
            Some((
                app_id,
                OpenAppWindows {
                    latest_focused_window: latest_window.id,
                    window_count: app_windows.len(),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use freedesktop_desktop_entry::DesktopEntry;

    use super::{AppCategory, LaunchableApp, OpenAppWindows, open_windows_by_app};
    use crate::desktop_windows_apps::{
        compositor_test_windows::unplaced_compositor_window,
        compositor_window_state::CompositorWindow, desktop_entry_app_icons::DesktopEntries,
    };

    fn entry_of(app_id: &str, fields: &str) -> DesktopEntry {
        let no_locales: Option<&[&str]> = None;
        DesktopEntry::from_str(
            format!("/{app_id}.desktop"),
            &format!("[Desktop Entry]\nType=Application\nName={app_id}\nExec={app_id}\n{fields}\n"),
            no_locales,
        )
        .expect("the fixture is a valid desktop entry")
    }

    fn app_on_niri(fields: &str) -> Option<LaunchableApp> {
        LaunchableApp::from_shown_entry(&entry_of("editor", fields), &["niri".to_owned()])
    }

    fn category_of(fields: &str) -> AppCategory {
        app_on_niri(fields).expect("the entry is shown").category
    }

    fn window_of(id: u64, app_id: &str, focused_at_secs: Option<u64>) -> CompositorWindow {
        CompositorWindow {
            app_id: Some(app_id.to_owned()),
            focused_at_monotonic: focused_at_secs.map(Duration::from_secs),
            ..unplaced_compositor_window(id)
        }
    }

    #[test]
    fn an_app_lands_in_the_first_main_category_it_declares() {
        assert_eq!(
            category_of("Categories=IDE;Office;Development;"),
            AppCategory::Office
        );
    }

    #[test]
    fn audio_and_video_share_the_multimedia_category() {
        assert_eq!(category_of("Categories=Audio;"), AppCategory::Multimedia);
        assert_eq!(category_of("Categories=Video;"), AppCategory::Multimedia);
        assert_eq!(
            category_of("Categories=AudioVideo;"),
            AppCategory::Multimedia
        );
    }

    #[test]
    fn an_app_without_a_main_category_lands_in_other() {
        assert_eq!(category_of("Categories=IDE;"), AppCategory::Other);
        assert_eq!(category_of(""), AppCategory::Other);
    }

    #[test]
    fn hidden_entries_are_not_launchable() {
        assert!(app_on_niri("NoDisplay=true").is_none());
        assert!(app_on_niri("Hidden=true").is_none());
    }

    #[test]
    fn desktop_lists_decide_whether_an_entry_is_shown() {
        assert!(app_on_niri("OnlyShowIn=KDE;").is_none());
        assert!(app_on_niri("OnlyShowIn=NIRI;").is_some());
        assert!(app_on_niri("NotShowIn=niri;").is_none());
        assert!(app_on_niri("NotShowIn=KDE;").is_some());
    }

    #[test]
    fn a_terminal_entry_keeps_its_command_and_flag() {
        let app = app_on_niri("Terminal=true").expect("the entry is shown");
        assert!(app.runs_in_terminal);
        assert_eq!(app.launch_command, ["editor"]);
    }

    #[test]
    fn the_catalog_lists_an_id_once_and_sorts_by_name_ignoring_case() {
        let entries = DesktopEntries::with_entries(vec![
            entry_of("zeal", ""),
            entry_of("Atom", ""),
            entry_of("zeal", "Name=Shadowed copy"),
            entry_of("mpv", ""),
        ]);

        let app_ids: Vec<String> = entries
            .launchable_apps()
            .into_iter()
            .map(|app| app.app_id)
            .collect();

        assert_eq!(app_ids, ["Atom", "mpv", "zeal"]);
    }

    #[test]
    fn the_most_recently_focused_window_is_the_jump_target() {
        let entries = DesktopEntries::with_entries(vec![entry_of("firefox", "")]);
        let windows = [
            window_of(1, "firefox", Some(10)),
            window_of(2, "firefox", Some(30)),
            window_of(3, "firefox", None),
            window_of(4, "not-an-entry", Some(50)),
        ];

        let open_windows = open_windows_by_app(&windows, &entries);

        assert_eq!(open_windows.len(), 1);
        assert_eq!(
            open_windows["firefox"],
            OpenAppWindows {
                latest_focused_window: 2,
                window_count: 3,
            }
        );
    }
}
