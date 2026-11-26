mod app_launcher_menu_entries;
pub mod app_ranking;
pub mod launcher_options;

use corredo::popovers::{NestedMenuAction, NestedMenuState};
use iced::Task;
use sottomano_compositor_model::compositor_requests::CompositorRequest;
use sottomano_compositors::compositor_connection::send_compositor_request;
use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
    AppCategory, AppDescription, LaunchSurface, LaunchableApp,
};
use sottomano_toml_config::Config;

use self::{
    app_ranking::{AppLaunchUsage, FrequentAppScope, apps_shown_in_category, frequent_apps_shown},
    launcher_options::{CategoryAppOrder, OpenAppListing},
};
use crate::{
    background_work::logged_failure_task,
    group_indicators::{
        indicator_group_content::{IndicatorGroupExpansion, IndicatorGroupExpansionMessage},
        sqlite_singleton_flag,
        terminal_command::TerminalCommand,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLauncherSubmenu {
    Frequent,
    Category(AppCategory),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum InspectedApp {
    #[default]
    Nothing,
    App(String),
}

#[derive(Debug, Clone)]
pub enum AppLauncherMessage {
    Menu(NestedMenuAction<AppLauncherSubmenu>),
    InspectApp(String),
    Launch(String),
    JumpToWindow(u64),
    ToggleHideOpenApps,
    ToggleSortByUsage,
    HideOpenAppsLoaded(OpenAppListing),
    SortByUsageLoaded(CategoryAppOrder),
    SettingsExpansion(IndicatorGroupExpansionMessage),
}

pub enum Action {
    Run(Task<AppLauncherMessage>),
    RunAndDismissMenu(Task<AppLauncherMessage>),
}

struct AppCategoryGroup {
    category: AppCategory,
    shown_apps: Vec<LaunchableApp>,
}

pub struct AppLauncherMenu {
    catalog: Vec<LaunchableApp>,
    usage: AppLaunchUsage,
    pub(crate) open_app_listing: OpenAppListing,
    pub(crate) category_app_order: CategoryAppOrder,
    pub(crate) settings_expansion: IndicatorGroupExpansion,
    category_groups: Vec<AppCategoryGroup>,
    pub menu_state: NestedMenuState<AppLauncherSubmenu>,
    inspected_app: InspectedApp,
}

impl Default for AppLauncherMenu {
    fn default() -> Self {
        Self {
            catalog: Vec::new(),
            usage: AppLaunchUsage::default(),
            open_app_listing: OpenAppListing::Listed,
            category_app_order: CategoryAppOrder::Alphabetical,
            settings_expansion: IndicatorGroupExpansion::default(),
            category_groups: AppCategory::ALL
                .into_iter()
                .map(|category| AppCategoryGroup {
                    category,
                    shown_apps: Vec::new(),
                })
                .collect(),
            menu_state: NestedMenuState::default(),
            inspected_app: InspectedApp::Nothing,
        }
    }
}

fn launch_task(app: &LaunchableApp, config: &Config) -> Task<AppLauncherMessage> {
    let launch_command = app.launch_command.clone();
    match app.launch_surface {
        LaunchSurface::Terminal => TerminalCommand::running(config, launch_command).open(),
        LaunchSurface::Graphical => logged_failure_task(
            "launch an app".to_owned(),
            send_compositor_request(CompositorRequest::Spawn(launch_command)),
        ),
    }
}

fn jump_task(window: u64) -> Task<AppLauncherMessage> {
    logged_failure_task(
        "jump to a window".to_owned(),
        send_compositor_request(CompositorRequest::FocusWindow(window)),
    )
}

impl AppLauncherMenu {
    pub fn load() -> Task<AppLauncherMessage> {
        Task::batch([
            Task::perform(
                sqlite_singleton_flag::load(OpenAppListing::Listed),
                AppLauncherMessage::HideOpenAppsLoaded,
            ),
            Task::perform(
                sqlite_singleton_flag::load(CategoryAppOrder::Alphabetical),
                AppLauncherMessage::SortByUsageLoaded,
            ),
        ])
    }

    pub fn collapse_settings(&mut self) {
        self.settings_expansion.collapse();
    }

    pub fn close_menu(&mut self) {
        self.menu_state.close();
        self.inspected_app = InspectedApp::Nothing;
    }

    pub fn adopt_catalog(&mut self, catalog: Vec<LaunchableApp>) {
        self.catalog = catalog;
        self.rebuild_category_groups();
    }

    pub fn adopt_usage(&mut self, usage: AppLaunchUsage) {
        self.usage = usage;
        self.rebuild_category_groups();
    }

    #[must_use]
    pub fn catalog_app_ids(&self) -> Vec<String> {
        self.catalog.iter().map(|app| app.app_id.clone()).collect()
    }

    fn rebuild_category_groups(&mut self) {
        let usage_order = match self.category_app_order {
            CategoryAppOrder::ByUsage => Some(self.usage.most_used_app_ids.as_slice()),
            CategoryAppOrder::Alphabetical => None,
        };
        for group in &mut self.category_groups {
            group.shown_apps = apps_shown_in_category(&self.catalog, group.category, usage_order);
        }
    }

    fn app_hover_card(&self, app_id: &str) -> Vec<Section> {
        let Some(app) = self.catalog.iter().find(|app| app.app_id == app_id) else {
            return Vec::new();
        };
        let mut entry_rows = vec![LabeledValue::new("App", app.name.clone())];
        if let AppDescription::Described(description) = &app.description {
            entry_rows.push(LabeledValue::new("Description", description.clone()));
        }
        if let Some(open_windows) = self.usage.open_windows.get(app_id) {
            entry_rows.push(LabeledValue::new(
                "Open windows",
                open_windows.window_count.to_string(),
            ));
        }
        if app.launch_surface == LaunchSurface::Terminal {
            entry_rows.push(LabeledValue::new("Runs in", "a terminal".to_owned()));
        }
        vec![Section::new("Desktop entry", dictionary_table(entry_rows))]
    }

    pub fn update(&mut self, message: AppLauncherMessage, config: &Config) -> Action {
        match message {
            AppLauncherMessage::Menu(menu_action) => {
                if let NestedMenuAction::Opened(open_submenus) = &menu_action
                    && *open_submenus != self.menu_state.open_submenus
                {
                    self.inspected_app = InspectedApp::Nothing;
                }
                self.menu_state.perform(menu_action);
                Action::Run(Task::none())
            }
            AppLauncherMessage::InspectApp(app_id) => {
                self.inspected_app = InspectedApp::App(app_id);
                Action::Run(Task::none())
            }
            AppLauncherMessage::Launch(app_id) => self
                .catalog
                .iter()
                .find(|app| app.app_id == app_id)
                .map_or(Action::Run(Task::none()), |app| {
                    Action::RunAndDismissMenu(launch_task(app, config))
                }),
            AppLauncherMessage::JumpToWindow(window) => {
                Action::RunAndDismissMenu(jump_task(window))
            }
            AppLauncherMessage::ToggleHideOpenApps => {
                self.open_app_listing = self.open_app_listing.toggled();
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.open_app_listing),
                ))
            }
            AppLauncherMessage::ToggleSortByUsage => {
                self.category_app_order = self.category_app_order.toggled();
                self.rebuild_category_groups();
                Action::Run(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.category_app_order),
                ))
            }
            AppLauncherMessage::HideOpenAppsLoaded(open_app_listing) => {
                self.open_app_listing = open_app_listing;
                Action::Run(Task::none())
            }
            AppLauncherMessage::SortByUsageLoaded(category_app_order) => {
                self.category_app_order = category_app_order;
                self.rebuild_category_groups();
                Action::Run(Task::none())
            }
            AppLauncherMessage::SettingsExpansion(expansion_message) => {
                self.settings_expansion.update(expansion_message);
                Action::Run(Task::none())
            }
        }
    }

    pub(super) fn frequent_scope(&self, output: &str) -> FrequentAppScope {
        self.usage
            .frequent_by_output
            .get(output)
            .map_or(FrequentAppScope::ThisWorkspace, |frequent| frequent.scope)
    }

    pub(super) fn frequent_apps_on(&self, output: &str) -> impl Iterator<Item = &LaunchableApp> {
        let frequent_apps = self
            .usage
            .frequent_by_output
            .get(output)
            .map_or(&[][..], |frequent| frequent.apps.as_slice());
        frequent_apps_shown(frequent_apps, self.open_app_listing)
    }
}

#[cfg(test)]
mod app_launcher_menu_tests;
