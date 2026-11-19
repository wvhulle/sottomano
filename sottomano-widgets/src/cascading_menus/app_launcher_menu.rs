use std::{cmp::Reverse, collections::HashMap};

use corredo::{
    popovers::{
        MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuSubmenu, NestedMenuAction,
        NestedMenuState,
    },
    theming::{primary_color, text_color},
    value_controls::SwitchResponse,
};
use iced::{Element, Task};
use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::{
    CompositorRequest, send_compositor_request,
};
use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
    AppCategory, LaunchableApp, OpenAppWindows,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::logged_failure_task,
    desktop_index::DesktopIndex,
    group_indicators::{
        indicator_toggle_switch::indicator_toggle_switch,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::GroupEvent,
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
        terminal_command::TerminalCommand,
    },
    widgets::{
        glyph_button,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const MAX_FREQUENT_APPS: usize = 8;
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
pub enum OpenAppListing {
    Hidden,
    Listed,
}

impl OpenAppListing {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            OpenAppListing::Hidden => OpenAppListing::Listed,
            OpenAppListing::Listed => OpenAppListing::Hidden,
        }
    }
}

impl SingletonFlagState for OpenAppListing {
    const STORED_FLAG: SingletonFlag = SingletonFlag::FrequentAppsHideOpen;
    const STORED_AS_SET: Self = OpenAppListing::Hidden;
    const STORED_AS_CLEARED: Self = OpenAppListing::Listed;
}

impl From<OpenAppListing> for SwitchPosition {
    fn from(open_app_listing: OpenAppListing) -> Self {
        match open_app_listing {
            OpenAppListing::Hidden => SwitchPosition::On,
            OpenAppListing::Listed => SwitchPosition::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryAppOrder {
    ByUsage,
    Alphabetical,
}

impl CategoryAppOrder {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            CategoryAppOrder::ByUsage => CategoryAppOrder::Alphabetical,
            CategoryAppOrder::Alphabetical => CategoryAppOrder::ByUsage,
        }
    }
}

impl SingletonFlagState for CategoryAppOrder {
    const STORED_FLAG: SingletonFlag = SingletonFlag::CategoryAppsSortedByUsage;
    const STORED_AS_SET: Self = CategoryAppOrder::ByUsage;
    const STORED_AS_CLEARED: Self = CategoryAppOrder::Alphabetical;
}

impl From<CategoryAppOrder> for SwitchPosition {
    fn from(category_app_order: CategoryAppOrder) -> Self {
        match category_app_order {
            CategoryAppOrder::ByUsage => SwitchPosition::On,
            CategoryAppOrder::Alphabetical => SwitchPosition::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequentAppScope {
    ThisWorkspace,
    AllWorkspaces,
}

impl FrequentAppScope {
    const fn submenu_title(self) -> &'static str {
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

fn hide_open_apps_toggle(open_app_listing: OpenAppListing) -> IconToggle {
    IconToggle::new(
        "Hide apps already open",
        Phosphor::EyeSlash(Weight::Regular),
        open_app_listing,
        SwitchPresses {
            turning_on: "leave out frequent apps with a window on this workspace",
            turning_off: "list frequent apps even with a window on this workspace",
        },
    )
}

fn sort_by_usage_toggle(category_app_order: CategoryAppOrder) -> IconToggle {
    IconToggle::new(
        "Sort categories by usage",
        Phosphor::SortAscending(Weight::Regular),
        category_app_order,
        SwitchPresses {
            turning_on: "list the most launched apps first in each category",
            turning_off: "list the apps in each category alphabetically",
        },
    )
}

fn apps_shown_in_category(
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

fn frequent_apps_shown(
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

    fn option_entry<'a>(
        toggle: IconToggle,
        message: AppLauncherMessage,
        config: &Config,
    ) -> MenuEntry<'a, AppLauncherSubmenu, GroupEvent> {
        let content = MenuRowContent::new(toggle.name)
            .icon(IconGlyph::from(toggle.icon).view(config.theme.font.icon_size_px, text_color))
            .value(indicator_toggle_switch(
                config,
                toggle.switch_position,
                SwitchResponse::Inert,
                1.0,
            ));
        MenuItem::new(MenuRow::new(content).on_press(GroupEvent::from(message))).into()
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
            [
                Self::option_entry(
                    hide_open_apps_toggle(self.open_app_listing),
                    AppLauncherMessage::ToggleHideOpenApps,
                    config,
                ),
                Self::option_entry(
                    sort_by_usage_toggle(self.category_app_order),
                    AppLauncherMessage::ToggleSortByUsage,
                    config,
                ),
            ],
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
mod tests {
    use std::{collections::HashMap, iter::once};

    use sottomano_desktop_sources::desktop_windows_apps::desktop_entry_app_catalog::{
        AppCategory, LaunchableApp, OpenAppWindows,
    };
    use sottomano_toml_config::Config;

    use super::{
        Action, AppLaunchUsage, AppLauncherMenu, AppLauncherMessage, FrequentApp, FrequentAppScope,
        MAX_FREQUENT_APPS, OpenAppListing, OutputFrequentApps, WorkspaceWindowPresence,
        apps_shown_in_category, frequent_apps_shown,
    };

    fn app_named(app_id: &str, category: AppCategory) -> LaunchableApp {
        LaunchableApp {
            app_id: app_id.to_owned(),
            name: app_id.to_owned(),
            description: None,
            category,
            launch_command: vec![app_id.to_owned()],
            runs_in_terminal: false,
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
        let frequent_apps: Vec<FrequentApp> =
            once(frequent_app("open", WorkspaceWindowPresence::Open))
                .chain((0..MAX_FREQUENT_APPS).map(|position| {
                    frequent_app(&format!("app{position}"), WorkspaceWindowPresence::Absent)
                }))
                .collect();

        let shown_with_hiding: Vec<&str> =
            frequent_apps_shown(&frequent_apps, OpenAppListing::Hidden)
                .map(|app| app.app_id.as_str())
                .collect();
        assert_eq!(shown_with_hiding.len(), MAX_FREQUENT_APPS);
        assert!(!shown_with_hiding.contains(&"open"));

        let first_without_hiding =
            frequent_apps_shown(&frequent_apps, OpenAppListing::Listed).next();
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
}
