pub mod app_ranking;
pub mod launcher_options;

use corredo::{
    popovers::{
        MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuSubmenu, NestedMenuAction,
        NestedMenuState,
    },
    theming::{primary_color, text_color},
};
use iced::{Element, Task};
use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::{
    CompositorRequest, send_compositor_request,
};
use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
    AppCategory, LaunchableApp,
};
use sottomano_toml_config::Config;

use self::{
    app_ranking::{AppLaunchUsage, FrequentAppScope, apps_shown_in_category, frequent_apps_shown},
    launcher_options::{CategoryAppOrder, OpenAppListing, option_entries},
};
use crate::{
    background_work::logged_failure_task,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    icon_catalog::drawable_icon_catalog::DesktopIndex,
    indicator_groups::{
        indicator_group_content::GroupEvent, sqlite_singleton_flag,
        terminal_command::TerminalCommand,
    },
    widgets::{glyph_button, value_cells::LabeledValue},
};

const OPTIONS_SUBMENU_TITLE: &str = "Launcher options";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLauncherSubmenu {
    Frequent,
    Category(AppCategory),
    Options,
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
    open_app_listing: OpenAppListing,
    category_app_order: CategoryAppOrder,
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

const fn category_glyph(category: AppCategory) -> Phosphor {
    match category {
        AppCategory::Development => Phosphor::Code(Weight::Regular),
        AppCategory::Education => Phosphor::GraduationCap(Weight::Regular),
        AppCategory::Game => Phosphor::GameController(Weight::Regular),
        AppCategory::Graphics => Phosphor::Palette(Weight::Regular),
        AppCategory::Multimedia => Phosphor::FilmStrip(Weight::Regular),
        AppCategory::Network => Phosphor::Globe(Weight::Regular),
        AppCategory::Office => Phosphor::Briefcase(Weight::Regular),
        AppCategory::Science => Phosphor::Atom(Weight::Regular),
        AppCategory::Settings => Phosphor::Gear(Weight::Regular),
        AppCategory::System => Phosphor::Cpu(Weight::Regular),
        AppCategory::Utility => Phosphor::Wrench(Weight::Regular),
        AppCategory::Other => Phosphor::DotsThree(Weight::Regular),
    }
}

fn launch_task(app: &LaunchableApp, config: &Config) -> Task<AppLauncherMessage> {
    let launch_command = app.launch_command.clone();
    if app.runs_in_terminal {
        return TerminalCommand::running(config, launch_command).open();
    }
    logged_failure_task(
        "launch an app".to_owned(),
        send_compositor_request(CompositorRequest::Spawn(launch_command)),
    )
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
        let description = app
            .description
            .clone()
            .map(|description| LabeledValue::new("Description", description));
        let open_window_count = self.usage.open_windows.get(app_id).map(|open_windows| {
            LabeledValue::new("Open windows", open_windows.window_count.to_string())
        });
        let runs_in_terminal = app
            .runs_in_terminal
            .then(|| LabeledValue::new("Runs in", "a terminal".to_owned()));
        vec![Section::new(
            "Desktop entry",
            dictionary_table(
                [LabeledValue::new("App", app.name.clone())]
                    .into_iter()
                    .chain(description)
                    .chain(open_window_count)
                    .chain(runs_in_terminal),
            ),
        )]
        .into_iter()
        .collect()
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
        }
    }

    fn jump_button<'a>(
        &self,
        app: &LaunchableApp,
        config: &Config,
    ) -> Option<Element<'a, GroupEvent>> {
        let open_windows = self.usage.open_windows.get(&app.app_id)?;
        Some(glyph_button(
            Phosphor::ArrowSquareOut(Weight::Regular),
            config.theme.font.icon_size_px,
            primary_color,
            GroupEvent::from(AppLauncherMessage::JumpToWindow(
                open_windows.latest_focused_window,
            )),
        ))
    }

    fn app_entry<'a>(
        &self,
        app: &LaunchableApp,
        index: &DesktopIndex,
        config: &Config,
        card_view: &dyn Fn(Vec<Section>) -> Option<Element<'a, GroupEvent>>,
    ) -> MenuEntry<'a, AppLauncherSubmenu, GroupEvent> {
        let icon_size_px = config.theme.font.icon_size_px;
        let content = MenuRowContent::new(app.name.clone());
        let content = match index.app_icons.get(&app.app_id) {
            Some(icon_handle) => content.icon(icon_handle.view(icon_size_px, 1.0)),
            None => content,
        };
        let content = match self.jump_button(app, config) {
            Some(jump_button) => content.value(jump_button),
            None => content,
        };
        let row = MenuRow::new(content)
            .on_press(GroupEvent::from(AppLauncherMessage::Launch(
                app.app_id.clone(),
            )))
            .on_enter(GroupEvent::from(AppLauncherMessage::InspectApp(
                app.app_id.clone(),
            )));
        let row = match self.usage.open_windows.get(&app.app_id) {
            Some(open_windows) => row.on_right_press(GroupEvent::from(
                AppLauncherMessage::JumpToWindow(open_windows.latest_focused_window),
            )),
            None => row,
        };
        let item = MenuItem::new(row);
        let inspected_card = match &self.inspected_app {
            InspectedApp::App(app_id) if *app_id == app.app_id => {
                card_view(self.app_hover_card(app_id))
            }
            InspectedApp::App(_) | InspectedApp::Nothing => None,
        };
        match inspected_card {
            Some(card) => item.beside(card),
            None => item,
        }
        .into()
    }

    fn frequent_scope(&self, output: &str) -> FrequentAppScope {
        self.usage
            .frequent_by_output
            .get(output)
            .map_or(FrequentAppScope::ThisWorkspace, |frequent| frequent.scope)
    }

    fn frequent_apps_on(&self, output: &str) -> impl Iterator<Item = &LaunchableApp> {
        let frequent_apps = self
            .usage
            .frequent_by_output
            .get(output)
            .map_or(&[][..], |frequent| frequent.apps.as_slice());
        frequent_apps_shown(frequent_apps, self.open_app_listing)
    }

    fn submenu_header<'a>(
        title: &'static str,
        glyph: Phosphor,
        config: &Config,
    ) -> MenuRowContent<'a, GroupEvent> {
        MenuRowContent::new(title)
            .icon(IconGlyph::from(glyph).view(config.theme.font.icon_size_px, text_color))
    }

    #[must_use]
    pub fn menu_entries<'a>(
        &self,
        index: &DesktopIndex,
        config: &Config,
        output: &str,
        card_view: &dyn Fn(Vec<Section>) -> Option<Element<'a, GroupEvent>>,
    ) -> Vec<MenuEntry<'a, AppLauncherSubmenu, GroupEvent>> {
        let frequent_entries: Vec<_> = self
            .frequent_apps_on(output)
            .map(|app| self.app_entry(app, index, config, card_view))
            .collect();
        let frequent_submenu = (!frequent_entries.is_empty()).then(|| {
            MenuSubmenu::new(
                AppLauncherSubmenu::Frequent,
                Self::submenu_header(
                    self.frequent_scope(output).submenu_title(),
                    Phosphor::ClockCounterClockwise(Weight::Regular),
                    config,
                ),
                frequent_entries,
            )
            .into()
        });
        let category_submenus = self
            .category_groups
            .iter()
            .filter(|group| !group.shown_apps.is_empty())
            .map(|group| {
                MenuSubmenu::new(
                    AppLauncherSubmenu::Category(group.category),
                    Self::submenu_header(
                        group.category.title(),
                        category_glyph(group.category),
                        config,
                    ),
                    group
                        .shown_apps
                        .iter()
                        .map(|app| self.app_entry(app, index, config, card_view)),
                )
                .into()
            });
        let options_submenu = MenuSubmenu::new(
            AppLauncherSubmenu::Options,
            Self::submenu_header(
                OPTIONS_SUBMENU_TITLE,
                Phosphor::SlidersHorizontal(Weight::Regular),
                config,
            ),
            option_entries(self.open_app_listing, self.category_app_order, config),
        )
        .into();
        frequent_submenu
            .into_iter()
            .chain(category_submenus)
            .chain([options_submenu])
            .collect()
    }
}

#[cfg(test)]
mod app_launcher_menu_tests;
