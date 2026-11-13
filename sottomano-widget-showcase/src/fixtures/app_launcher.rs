use std::{collections::HashMap, path::Path};

use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
    AppCategory, LaunchableApp, OpenAppWindows,
};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    group_menus::group_menu_state::{GroupMenuMessage, GroupMenuTopic},
    widget_groups::app_launcher_group::{
        AppLaunchUsage, AppLauncherGroup, FrequentApp, FrequentAppScope, OutputFrequentApps,
        WorkspaceWindowPresence,
    },
    widgets::icon_handle::IconHandle,
};

use crate::{
    showcase_message::ShowcaseMessage,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedAppLauncher, ShowcasedGroup},
};

const SHOWCASE_APP_ICONS: &str = env!("SHOWCASE_APP_ICONS");
const FIREFOX_WINDOW: u64 = 11;
const KITTY_WINDOW: u64 = 12;

struct CatalogApp {
    app_id: &'static str,
    name: &'static str,
    description: &'static str,
    category: AppCategory,
}

const fn catalog_app(
    app_id: &'static str,
    name: &'static str,
    description: &'static str,
    category: AppCategory,
) -> CatalogApp {
    CatalogApp {
        app_id,
        name,
        description,
        category,
    }
}

const CATALOG: &[CatalogApp] = &[
    catalog_app(
        "android-studio",
        "Android Studio",
        "Android development environment",
        AppCategory::Development,
    ),
    catalog_app("clion", "CLion", "C and C++ IDE", AppCategory::Development),
    catalog_app(
        "dbeaver",
        "DBeaver",
        "Database manager",
        AppCategory::Development,
    ),
    catalog_app("emacs", "Emacs", "Text editor", AppCategory::Development),
    catalog_app(
        "github-desktop",
        "GitHub Desktop",
        "Git client",
        AppCategory::Development,
    ),
    catalog_app(
        "gitg",
        "gitg",
        "Git repository viewer",
        AppCategory::Development,
    ),
    catalog_app(
        "gitkraken",
        "GitKraken",
        "Git client",
        AppCategory::Development,
    ),
    catalog_app("godot", "Godot", "Game engine", AppCategory::Development),
    catalog_app(
        "insomnia",
        "Insomnia",
        "API client",
        AppCategory::Development,
    ),
    catalog_app(
        "intellij-idea",
        "IntelliJ IDEA",
        "Java IDE",
        AppCategory::Development,
    ),
    catalog_app("kdevelop", "KDevelop", "IDE", AppCategory::Development),
    catalog_app("meld", "Meld", "Diff viewer", AppCategory::Development),
    catalog_app("postman", "Postman", "API client", AppCategory::Development),
    catalog_app("pycharm", "PyCharm", "Python IDE", AppCategory::Development),
    catalog_app(
        "qtcreator",
        "Qt Creator",
        "Qt development environment",
        AppCategory::Development,
    ),
    catalog_app(
        "rustrover",
        "RustRover",
        "Rust IDE",
        AppCategory::Development,
    ),
    catalog_app(
        "sublime-text",
        "Sublime Text",
        "Text editor",
        AppCategory::Development,
    ),
    catalog_app(
        "code",
        "Visual Studio Code",
        "Code editor",
        AppCategory::Development,
    ),
    catalog_app(
        "wireshark",
        "Wireshark",
        "Network protocol analyzer",
        AppCategory::Development,
    ),
    catalog_app("zed", "Zed", "Code editor", AppCategory::Development),
    catalog_app("steam", "Steam", "Game library", AppCategory::Game),
    catalog_app("gimp", "GIMP", "Image editor", AppCategory::Graphics),
    catalog_app(
        "org.inkscape.Inkscape",
        "Inkscape",
        "Vector graphics editor",
        AppCategory::Graphics,
    ),
    catalog_app(
        "blender",
        "Blender",
        "3D creation suite",
        AppCategory::Graphics,
    ),
    catalog_app("mpv", "mpv", "Media player", AppCategory::Multimedia),
    catalog_app(
        "spotify",
        "Spotify",
        "Music player",
        AppCategory::Multimedia,
    ),
    catalog_app("vlc", "VLC", "Media player", AppCategory::Multimedia),
    catalog_app("firefox", "Firefox", "Web browser", AppCategory::Network),
    catalog_app(
        "org.telegram.desktop",
        "Telegram",
        "Messaging app",
        AppCategory::Network,
    ),
    catalog_app(
        "thunderbird",
        "Thunderbird",
        "Email client",
        AppCategory::Network,
    ),
    catalog_app(
        "libreoffice-calc",
        "LibreOffice Calc",
        "Spreadsheet editor",
        AppCategory::Office,
    ),
    catalog_app(
        "libreoffice-writer",
        "LibreOffice Writer",
        "Document editor",
        AppCategory::Office,
    ),
    catalog_app("obsidian", "Obsidian", "Note taking", AppCategory::Office),
    catalog_app("kitty", "kitty", "Terminal emulator", AppCategory::System),
    catalog_app(
        "org.gnome.Nautilus",
        "Files",
        "File manager",
        AppCategory::Utility,
    ),
];

const MOST_LAUNCHED_APP_IDS: [&str; 5] = [
    "firefox",
    "kitty",
    "code",
    "obsidian",
    "org.telegram.desktop",
];

const LONG_UTILITY_APP_IDS: [&str; 40] = [
    "nautilus",
    "gedit",
    "gnome-calculator",
    "kcalc",
    "ark",
    "file-roller",
    "gparted",
    "baobab",
    "flameshot",
    "keepassxc",
    "1password",
    "bitwarden",
    "syncthing",
    "filezilla",
    "transmission",
    "qbittorrent",
    "gnome-disks",
    "seahorse",
    "dconf-editor",
    "htop",
    "nvtop",
    "gnome-system-monitor",
    "ksysguard",
    "kate",
    "kwrite",
    "xed",
    "mousepad",
    "evince",
    "okular",
    "zathura",
    "remmina",
    "virtualbox",
    "gnome-boxes",
    "timeshift",
    "bleachbit",
    "stacer",
    "peek",
    "obs",
    "nextcloud",
    "dropbox",
];
const SCROLLED_PAST_FIRST_ROWS_PX: f32 = 300.0;

fn long_utility_app(app_id: &str) -> LaunchableApp {
    LaunchableApp {
        app_id: app_id.to_owned(),
        name: app_id.to_owned(),
        description: None,
        category: AppCategory::Utility,
        launch_command: vec![app_id.to_owned()],
        runs_in_terminal: false,
    }
}

pub fn scrolled_item_list(_clock: Clock) -> Vec<ShowcaseMessage> {
    vec![ShowcaseMessage::GroupMenuState(
        GroupMenuMessage::ItemListScrolled {
            menu: GroupMenuTopic::Applications,
            scrolled_px: SCROLLED_PAST_FIRST_ROWS_PX,
        },
    )]
}

fn launchable_app(app: &CatalogApp) -> LaunchableApp {
    LaunchableApp {
        app_id: app.app_id.to_owned(),
        name: app.name.to_owned(),
        description: Some(app.description.to_owned()),
        category: app.category,
        launch_command: vec![app.app_id.to_owned()],
        runs_in_terminal: false,
    }
}

fn open_windows() -> HashMap<String, OpenAppWindows> {
    HashMap::from([
        (
            "firefox".to_owned(),
            OpenAppWindows {
                latest_focused_window: FIREFOX_WINDOW,
                window_count: 2,
            },
        ),
        (
            "kitty".to_owned(),
            OpenAppWindows {
                latest_focused_window: KITTY_WINDOW,
                window_count: 1,
            },
        ),
    ])
}

fn frequent_apps(open_windows: &HashMap<String, OpenAppWindows>) -> Vec<FrequentApp> {
    MOST_LAUNCHED_APP_IDS
        .iter()
        .filter_map(|app_id| CATALOG.iter().find(|app| app.app_id == *app_id))
        .map(|app| FrequentApp {
            app: launchable_app(app),
            window_presence: if open_windows.contains_key(app.app_id) {
                WorkspaceWindowPresence::Open
            } else {
                WorkspaceWindowPresence::Absent
            },
        })
        .collect()
}

pub fn group(_clock: Clock) -> ShowcasedGroup {
    let open_windows = open_windows();
    let mut group = AppLauncherGroup::default();
    group.adopt_catalog(
        CATALOG
            .iter()
            .map(launchable_app)
            .chain(LONG_UTILITY_APP_IDS.map(long_utility_app))
            .collect(),
    );
    group.adopt_usage(AppLaunchUsage {
        frequent_by_output: HashMap::from([(
            SHOWCASE_OUTPUT.to_owned(),
            OutputFrequentApps {
                scope: FrequentAppScope::ThisWorkspace,
                apps: frequent_apps(&open_windows),
            },
        )]),
        most_used_app_ids: MOST_LAUNCHED_APP_IDS.map(str::to_owned).to_vec(),
        open_windows,
    });
    ShowcasedGroup::AppLauncher(Box::new(ShowcasedAppLauncher {
        group,
        desktop_entries: DesktopIndex::with_app_icons(
            CATALOG
                .iter()
                .map(|app| app.app_id)
                .chain(LONG_UTILITY_APP_IDS)
                .map(|app_id| {
                    (
                        app_id.to_owned(),
                        IconHandle::at(
                            &Path::new(SHOWCASE_APP_ICONS).join(format!("{app_id}.svg")),
                        ),
                    )
                }),
        ),
    }))
}
