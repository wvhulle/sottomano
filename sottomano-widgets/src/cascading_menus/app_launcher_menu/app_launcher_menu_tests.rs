use std::{collections::HashMap, iter::once};

use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
    AppCategory, AppDescription, LaunchSurface, LaunchableApp, OpenAppWindows,
};
use sottomano_toml_config::Config;

use super::{
    Action, AppLauncherMenu, AppLauncherMessage,
    app_ranking::{
        AppLaunchUsage, FrequentApp, FrequentAppScope, MAX_FREQUENT_APPS, OutputFrequentApps,
        WorkspaceWindowPresence, apps_shown_in_category, frequent_apps_shown,
    },
    launcher_options::OpenAppListing,
};

fn app_named(app_id: &str, category: AppCategory) -> LaunchableApp {
    LaunchableApp {
        app_id: app_id.to_owned(),
        name: app_id.to_owned(),
        description: AppDescription::Undescribed,
        category,
        launch_command: vec![app_id.to_owned()],
        launch_surface: LaunchSurface::Graphical,
    }
}

fn development_app(app_id: &str) -> LaunchableApp {
    app_named(app_id, AppCategory::Development)
}

fn frequent_app(app_id: &str, window_presence: WorkspaceWindowPresence) -> FrequentApp {
    FrequentApp {
        app: development_app(app_id),
        window_presence,
    }
}

fn shown_ids(apps: Vec<LaunchableApp>) -> Vec<String> {
    apps.into_iter().map(|app| app.app_id).collect()
}

fn alphabetical_catalog() -> Vec<LaunchableApp> {
    vec![
        development_app("atom"),
        development_app("helix"),
        development_app("zed"),
        app_named("mpv", AppCategory::Multimedia),
    ]
}

#[test]
fn a_category_only_lists_its_own_apps_in_catalog_order() {
    assert_eq!(
        shown_ids(apps_shown_in_category(
            &alphabetical_catalog(),
            AppCategory::Development,
            None
        )),
        ["atom", "helix", "zed"]
    );
}

#[test]
fn sorting_by_usage_puts_used_apps_first_and_keeps_the_rest_alphabetical() {
    let most_used = ["zed".to_owned(), "helix".to_owned(), "mpv".to_owned()];
    let mut catalog = alphabetical_catalog();
    catalog.insert(1, development_app("emacs"));

    assert_eq!(
        shown_ids(apps_shown_in_category(
            &catalog,
            AppCategory::Development,
            Some(&most_used)
        )),
        ["zed", "helix", "atom", "emacs"]
    );
}

#[test]
fn hiding_open_apps_filters_before_the_frequent_cap() {
    let frequent_apps: Vec<FrequentApp> = once(frequent_app("open", WorkspaceWindowPresence::Open))
        .chain((0..MAX_FREQUENT_APPS).map(|position| {
            frequent_app(&format!("app{position}"), WorkspaceWindowPresence::Absent)
        }))
        .collect();

    let shown_with_hiding: Vec<&str> = frequent_apps_shown(&frequent_apps, OpenAppListing::Hidden)
        .map(|app| app.app_id.as_str())
        .collect();
    assert_eq!(shown_with_hiding.len(), MAX_FREQUENT_APPS);
    assert!(!shown_with_hiding.contains(&"open"));

    let first_without_hiding = frequent_apps_shown(&frequent_apps, OpenAppListing::Listed).next();
    assert_eq!(
        first_without_hiding.map(|app| app.app_id.as_str()),
        Some("open")
    );
}

#[test]
fn the_frequent_group_belongs_to_the_output_it_is_shown_on() {
    let mut group = AppLauncherMenu::default();
    group.adopt_usage(AppLaunchUsage {
        frequent_by_output: HashMap::from([(
            "DP-1".to_owned(),
            OutputFrequentApps {
                scope: FrequentAppScope::ThisWorkspace,
                apps: vec![frequent_app("helix", WorkspaceWindowPresence::Absent)],
            },
        )]),
        ..AppLaunchUsage::default()
    });
    let items_on = |output: &str| group.frequent_apps_on(output).next().is_some();

    assert!(items_on("DP-1"));
    assert!(!items_on("HDMI-1"));
}

#[test]
fn the_frequent_title_names_the_scope_it_ranks_over() {
    let frequent_title_for = |scope: FrequentAppScope| {
        let mut group = AppLauncherMenu::default();
        group.adopt_usage(AppLaunchUsage {
            frequent_by_output: HashMap::from([(
                "DP-1".to_owned(),
                OutputFrequentApps {
                    scope,
                    apps: vec![frequent_app("helix", WorkspaceWindowPresence::Absent)],
                },
            )]),
            ..AppLaunchUsage::default()
        });
        group.frequent_scope("DP-1").submenu_title()
    };

    assert_eq!(
        frequent_title_for(FrequentAppScope::ThisWorkspace),
        "Frequently launched on this workspace"
    );
    assert_eq!(
        frequent_title_for(FrequentAppScope::AllWorkspaces),
        "Frequently launched"
    );
}

#[test]
fn launching_and_jumping_dismiss_the_menu_but_toggles_do_not() {
    let mut group = AppLauncherMenu::default();
    group.adopt_catalog(alphabetical_catalog());
    let config = Config::default();

    assert!(matches!(
        group.update(AppLauncherMessage::Launch("helix".to_owned()), &config),
        Action::RunAndDismissMenu(_)
    ));
    assert!(matches!(
        group.update(AppLauncherMessage::JumpToWindow(7), &config),
        Action::RunAndDismissMenu(_)
    ));
    assert!(matches!(
        group.update(AppLauncherMessage::Launch("gone".to_owned()), &config),
        Action::Run(_)
    ));
    assert!(matches!(
        group.update(
            AppLauncherMessage::HideOpenAppsLoaded(OpenAppListing::Hidden),
            &config
        ),
        Action::Run(_)
    ));
}

#[test]
fn an_open_window_offers_a_jump_only_for_its_own_app() {
    let mut group = AppLauncherMenu::default();
    group.adopt_usage(AppLaunchUsage {
        open_windows: HashMap::from([(
            "helix".to_owned(),
            OpenAppWindows {
                latest_focused_window: 7,
                window_count: 2,
            },
        )]),
        ..AppLaunchUsage::default()
    });
    let config = Config::default();

    assert!(
        group
            .jump_button(&development_app("helix"), &config)
            .is_some()
    );
    assert!(
        group
            .jump_button(&development_app("zed"), &config)
            .is_none()
    );
}
