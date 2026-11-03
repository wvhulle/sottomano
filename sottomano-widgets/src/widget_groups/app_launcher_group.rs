use std::{cmp::Reverse, collections::HashMap};

use iced::{Element, Task};
use sottomano_config::Config;
use sottomano_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{
        desktop_entry_app_catalog::{AppCategory, LaunchableApp, OpenAppWindows},
        niri_action_requests,
    },
};

use crate::{
    clock::Clock,
    desktop_index::DesktopIndex,
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        sqlite_singleton_flag::{self, SingletonFlag},
        terminal_command::TerminalCommand,
        widget_group_content::{
            GroupEvent, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        configured_theme::primary_color,
        glyph_button,
        icon_glyph::IconGlyph,
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const MAX_FREQUENT_APPS: usize = 8;
const FREQUENT_GROUP_NAME: &str = "frequent";
const OPTIONS_GROUP_NAME: &str = "options";
const JUMP_PRESS_HINT: &str = "focus its most recently used window";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppLauncherTarget {
    App(String),
    HideOpenApps,
    SortByUsage,
}

impl From<AppLauncherTarget> for HoverTarget {
    fn from(target: AppLauncherTarget) -> Self {
        HoverTarget::AppLauncher(target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrequentApp {
    pub app: LaunchableApp,
    pub open_on_workspace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequentAppScope {
    ThisWorkspace,
    AllWorkspaces,
}

impl FrequentAppScope {
    const fn group_title(self) -> &'static str {
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
    FrequentExpansion(WidgetGroupExpansionMessage),
    CategoryExpansion(AppCategory, WidgetGroupExpansionMessage),
    OptionsExpansion(WidgetGroupExpansionMessage),
    Launch(String),
    JumpToWindow(u64),
    ToggleHideOpenApps,
    ToggleSortByUsage,
    HideOpenAppsLoaded(bool),
    SortByUsageLoaded(bool),
}

pub enum Action {
    Run(Task<AppLauncherMessage>),
    RunAndDismissMenu(Task<AppLauncherMessage>),
}

struct AppCategoryGroup {
    category: AppCategory,
    shown_apps: Vec<LaunchableApp>,
    expansion: WidgetGroupExpansion,
}

pub struct AppLauncherGroup {
    catalog: Vec<LaunchableApp>,
    usage: AppLaunchUsage,
    hide_open_apps: bool,
    sort_by_usage: bool,
    frequent_expansion: WidgetGroupExpansion,
    options_expansion: WidgetGroupExpansion,
    category_groups: Vec<AppCategoryGroup>,
}

impl Default for AppLauncherGroup {
    fn default() -> Self {
        Self {
            catalog: Vec::new(),
            usage: AppLaunchUsage::default(),
            hide_open_apps: false,
            sort_by_usage: false,
            frequent_expansion: WidgetGroupExpansion::default(),
            options_expansion: WidgetGroupExpansion::default(),
            category_groups: AppCategory::ALL
                .into_iter()
                .map(|category| AppCategoryGroup {
                    category,
                    shown_apps: Vec::new(),
                    expansion: WidgetGroupExpansion::default(),
                })
                .collect(),
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

fn hide_open_apps_toggle(hide_open_apps: bool) -> IconToggle {
    IconToggle::new(
        "Hide apps already open",
        Phosphor::EyeSlash(Weight::Regular),
        hide_open_apps,
        SwitchPresses {
            turning_on: "leave out frequent apps with a window on this workspace",
            turning_off: "list frequent apps even with a window on this workspace",
        },
    )
}

fn sort_by_usage_toggle(sort_by_usage: bool) -> IconToggle {
    IconToggle::new(
        "Sort categories by usage",
        Phosphor::SortAscending(Weight::Regular),
        sort_by_usage,
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
    hide_open_apps: bool,
) -> impl Iterator<Item = &LaunchableApp> {
    frequent_apps
        .iter()
        .filter(move |frequent| !(hide_open_apps && frequent.open_on_workspace))
        .map(|frequent| &frequent.app)
        .take(MAX_FREQUENT_APPS)
}

fn launch_task(app: &LaunchableApp, config: &Config) -> Task<AppLauncherMessage> {
    let launch_command = app.launch_command.clone();
    if app.runs_in_terminal {
        return TerminalCommand::running(config, launch_command).open();
    }
    Task::future(blocking("launch app", move || {
        niri_action_requests::send_action_or_warn(&niri_ipc::Action::Spawn {
            command: launch_command,
        });
    }))
    .discard()
}

fn jump_task(window: u64) -> Task<AppLauncherMessage> {
    Task::future(blocking("focus app window", move || {
        niri_action_requests::send_action_or_warn(&niri_ipc::Action::FocusWindow { id: window });
    }))
    .discard()
}

impl AppLauncherGroup {
    pub fn load() -> Task<AppLauncherMessage> {
        Task::batch([
            Task::perform(
                sqlite_singleton_flag::load(SingletonFlag::FrequentAppsHideOpen, false),
                AppLauncherMessage::HideOpenAppsLoaded,
            ),
            Task::perform(
                sqlite_singleton_flag::load(SingletonFlag::CategoryAppsSortedByUsage, false),
                AppLauncherMessage::SortByUsageLoaded,
            ),
        ])
    }

    pub fn collapse(&mut self) {
        self.frequent_expansion.collapse();
        self.options_expansion.collapse();
        for group in &mut self.category_groups {
            group.expansion.collapse();
        }
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
        let usage_order = self
            .sort_by_usage
            .then_some(self.usage.most_used_app_ids.as_slice());
        for group in &mut self.category_groups {
            group.shown_apps = apps_shown_in_category(&self.catalog, group.category, usage_order);
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &AppLauncherTarget, clock: Clock) -> HoverCard {
        match target {
            AppLauncherTarget::App(app_id) => self.app_hover_card(app_id),
            AppLauncherTarget::HideOpenApps => {
                hide_open_apps_toggle(self.hide_open_apps).switch_hover_card(clock)
            }
            AppLauncherTarget::SortByUsage => {
                sort_by_usage_toggle(self.sort_by_usage).switch_hover_card(clock)
            }
        }
    }

    fn app_hover_card(&self, app_id: &str) -> HoverCard {
        let Some(app) = self.catalog.iter().find(|app| app.app_id == app_id) else {
            return HoverCard::empty();
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
        HoverCard::sourced(
            HoverCardBody::dictionary(
                [LabeledValue::new("App", app.name.clone())]
                    .into_iter()
                    .chain(description)
                    .chain(open_window_count)
                    .chain(runs_in_terminal),
            ),
            DeliveryTiming::LivePush,
        )
    }

    pub fn update(&mut self, message: AppLauncherMessage, config: &Config) -> Action {
        match message {
            AppLauncherMessage::FrequentExpansion(expansion) => {
                self.frequent_expansion.update(expansion);
                Action::Run(Task::none())
            }
            AppLauncherMessage::OptionsExpansion(expansion) => {
                self.options_expansion.update(expansion);
                Action::Run(Task::none())
            }
            AppLauncherMessage::CategoryExpansion(category, expansion) => {
                if let Some(group) = self
                    .category_groups
                    .iter_mut()
                    .find(|group| group.category == category)
                {
                    group.expansion.update(expansion);
                }
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
                self.hide_open_apps = !self.hide_open_apps;
                Action::Run(
                    Task::future(sqlite_singleton_flag::save(
                        SingletonFlag::FrequentAppsHideOpen,
                        self.hide_open_apps,
                    ))
                    .discard(),
                )
            }
            AppLauncherMessage::ToggleSortByUsage => {
                self.sort_by_usage = !self.sort_by_usage;
                self.rebuild_category_groups();
                Action::Run(
                    Task::future(sqlite_singleton_flag::save(
                        SingletonFlag::CategoryAppsSortedByUsage,
                        self.sort_by_usage,
                    ))
                    .discard(),
                )
            }
            AppLauncherMessage::HideOpenAppsLoaded(hide_open_apps) => {
                self.hide_open_apps = hide_open_apps;
                Action::Run(Task::none())
            }
            AppLauncherMessage::SortByUsageLoaded(sort_by_usage) => {
                self.sort_by_usage = sort_by_usage;
                self.rebuild_category_groups();
                Action::Run(Task::none())
            }
        }
    }

    fn app_face(app: &LaunchableApp, index: &DesktopIndex) -> ComposedGroupItemFace {
        ComposedGroupItemFace {
            name: app.name.clone().into(),
            icon: index
                .app_icon(&app.app_id)
                .map_or(GroupItemIcon::Absent, |icon_path| {
                    GroupItemIcon::Image(IconHandle::at(icon_path))
                }),
        }
    }

    fn jump_indicator<'a>(
        &self,
        app: &LaunchableApp,
        config: &'a Config,
    ) -> GroupItemValue<'a, AppLauncherMessage> {
        self.usage
            .open_windows
            .get(&app.app_id)
            .map_or(GroupItemValue::Nothing, |open_windows| {
                let window = open_windows.latest_focused_window;
                GroupItemValue::drawn(
                    move |_reveal| -> Element<'a, GroupEvent<AppLauncherMessage>> {
                        glyph_button(
                            Phosphor::ArrowSquareOut(Weight::Regular),
                            config.theme.font.icon_size_px,
                            primary_color,
                            GroupEvent::Group(AppLauncherMessage::JumpToWindow(window)),
                        )
                    },
                )
            })
    }

    fn with_app_gestures<'a>(
        &self,
        row: GroupItem<'a, AppLauncherMessage>,
        app: &LaunchableApp,
    ) -> GroupItem<'a, AppLauncherMessage> {
        let launchable_row = row.on_press("launch", AppLauncherMessage::Launch(app.app_id.clone()));
        match self.usage.open_windows.get(&app.app_id) {
            Some(open_windows) => launchable_row.on_right_press(
                JUMP_PRESS_HINT,
                AppLauncherMessage::JumpToWindow(open_windows.latest_focused_window),
            ),
            None => launchable_row,
        }
    }

    fn frequent_row<'a>(
        &self,
        app: &LaunchableApp,
        index: &DesktopIndex,
        config: &'a Config,
    ) -> GroupItem<'a, AppLauncherMessage> {
        self.with_app_gestures(
            GroupItem::composed(
                Self::app_face(app, index),
                GroupItemHoverArea::WholeItem(AppLauncherTarget::App(app.app_id.clone()).into()),
                self.jump_indicator(app, config),
            ),
            app,
        )
    }

    fn category_row<'a>(
        &self,
        app: &LaunchableApp,
        index: &DesktopIndex,
        config: &'a Config,
    ) -> GroupItem<'a, AppLauncherMessage> {
        self.with_app_gestures(
            GroupItem::composed_pinnable(
                Self::app_face(app, index),
                AppLauncherTarget::App(app.app_id.clone()),
                self.jump_indicator(app, config),
            ),
            app,
        )
    }

    fn frequent_group<'a>(
        &'a self,
        index: &DesktopIndex,
        config: &'a Config,
        output: &str,
    ) -> WidgetGroupContent<'a, AppLauncherMessage> {
        let output_frequent_apps = self.usage.frequent_by_output.get(output);
        let frequent_apps =
            output_frequent_apps.map_or(&[][..], |frequent| frequent.apps.as_slice());
        let scope =
            output_frequent_apps.map_or(FrequentAppScope::ThisWorkspace, |frequent| frequent.scope);
        let frequent_rows: Vec<_> = frequent_apps_shown(frequent_apps, self.hide_open_apps)
            .map(|app| self.frequent_row(app, index, config))
            .collect();
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::AppLauncher, FREQUENT_GROUP_NAME),
            WidgetGroupTitle::fixed(scope.group_title()),
            &self.frequent_expansion,
            AppLauncherMessage::FrequentExpansion,
            frequent_rows,
        )
        .face(WidgetGroupFace::Icon {
            glyph: IconGlyph::from(Phosphor::ClockCounterClockwise(Weight::Regular)),
        })
    }

    fn category_group<'a>(
        &'a self,
        group: &'a AppCategoryGroup,
        index: &DesktopIndex,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, AppLauncherMessage> {
        let category = group.category;
        let category_rows: Vec<_> = group
            .shown_apps
            .iter()
            .map(|app| self.category_row(app, index, config))
            .collect();
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::AppLauncher, category.stored_name()),
            WidgetGroupTitle::fixed(category.title()),
            &group.expansion,
            move |expansion| AppLauncherMessage::CategoryExpansion(category, expansion),
            category_rows,
        )
        .face(WidgetGroupFace::Icon {
            glyph: IconGlyph::from(category_glyph(category)),
        })
    }

    fn options_group(&self) -> WidgetGroupContent<'_, AppLauncherMessage> {
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::AppLauncher, OPTIONS_GROUP_NAME),
            WidgetGroupTitle::fixed("Launcher options"),
            &self.options_expansion,
            AppLauncherMessage::OptionsExpansion,
            [
                GroupItem::toggle(
                    hide_open_apps_toggle(self.hide_open_apps),
                    AppLauncherTarget::HideOpenApps,
                    AppLauncherMessage::ToggleHideOpenApps,
                ),
                GroupItem::toggle(
                    sort_by_usage_toggle(self.sort_by_usage),
                    AppLauncherTarget::SortByUsage,
                    AppLauncherMessage::ToggleSortByUsage,
                ),
            ],
        )
        .face(WidgetGroupFace::Icon {
            glyph: IconGlyph::from(Phosphor::SlidersHorizontal(Weight::Regular)),
        })
    }

    #[must_use]
    pub fn group_contents<'a>(
        &'a self,
        index: &DesktopIndex,
        config: &'a Config,
        output: &str,
    ) -> Vec<WidgetGroupContent<'a, AppLauncherMessage>> {
        [self.frequent_group(index, config, output)]
            .into_iter()
            .chain(
                self.category_groups
                    .iter()
                    .map(|group| self.category_group(group, index, config)),
            )
            .chain([self.options_group()])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, iter::once};

    use sottomano_config::Config;
    use sottomano_sources::desktop_windows_apps::desktop_entry_app_catalog::{
        AppCategory, LaunchableApp, OpenAppWindows,
    };

    use super::{
        Action, AppLaunchUsage, AppLauncherGroup, AppLauncherMessage, FrequentApp,
        FrequentAppScope, GroupItemValue, MAX_FREQUENT_APPS, OutputFrequentApps,
        apps_shown_in_category, frequent_apps_shown,
    };
    use crate::desktop_index::DesktopIndex;

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

    fn frequent_app(app_id: &str, open_on_workspace: bool) -> FrequentApp {
        FrequentApp {
            app: development_app(app_id),
            open_on_workspace,
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
        let frequent_apps: Vec<FrequentApp> = once(frequent_app("open", true))
            .chain(
                (0..MAX_FREQUENT_APPS)
                    .map(|position| frequent_app(&format!("app{position}"), false)),
            )
            .collect();

        let shown_with_hiding: Vec<&str> = frequent_apps_shown(&frequent_apps, true)
            .map(|app| app.app_id.as_str())
            .collect();
        assert_eq!(shown_with_hiding.len(), MAX_FREQUENT_APPS);
        assert!(!shown_with_hiding.contains(&"open"));

        let first_without_hiding = frequent_apps_shown(&frequent_apps, false).next();
        assert_eq!(
            first_without_hiding.map(|app| app.app_id.as_str()),
            Some("open")
        );
    }

    #[test]
    fn the_frequent_group_belongs_to_the_output_it_is_shown_on() {
        let mut group = AppLauncherGroup::default();
        group.adopt_usage(AppLaunchUsage {
            frequent_by_output: HashMap::from([(
                "DP-1".to_owned(),
                OutputFrequentApps {
                    scope: FrequentAppScope::ThisWorkspace,
                    apps: vec![frequent_app("helix", false)],
                },
            )]),
            ..AppLaunchUsage::default()
        });
        let config = Config::default();
        let index = DesktopIndex::default();

        let items_on = |output: &str| {
            group
                .group_contents(&index, &config, output)
                .into_iter()
                .next()
                .is_some_and(|frequent| frequent.has_items())
        };

        assert!(items_on("DP-1"));
        assert!(!items_on("HDMI-1"));
    }

    #[test]
    fn the_frequent_title_names_the_scope_it_ranks_over() {
        let frequent_title_for = |scope: FrequentAppScope| {
            let mut group = AppLauncherGroup::default();
            group.adopt_usage(AppLaunchUsage {
                frequent_by_output: HashMap::from([(
                    "DP-1".to_owned(),
                    OutputFrequentApps {
                        scope,
                        apps: vec![frequent_app("helix", false)],
                    },
                )]),
                ..AppLaunchUsage::default()
            });
            let config = Config::default();
            group
                .group_contents(&DesktopIndex::default(), &config, "DP-1")
                .first()
                .map(|frequent| frequent.group_title().to_string())
        };

        assert_eq!(
            frequent_title_for(FrequentAppScope::ThisWorkspace).as_deref(),
            Some("Frequently launched on this workspace")
        );
        assert_eq!(
            frequent_title_for(FrequentAppScope::AllWorkspaces).as_deref(),
            Some("Frequently launched")
        );
    }

    #[test]
    fn launching_and_jumping_dismiss_the_menu_but_toggles_do_not() {
        let mut group = AppLauncherGroup::default();
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
            group.update(AppLauncherMessage::HideOpenAppsLoaded(true), &config),
            Action::Run(_)
        ));
    }

    #[test]
    fn an_open_window_offers_a_jump_only_for_its_own_app() {
        let mut group = AppLauncherGroup::default();
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

        assert!(matches!(
            group.jump_indicator(&development_app("helix"), &config),
            GroupItemValue::Drawn(_)
        ));
        assert!(matches!(
            group.jump_indicator(&development_app("zed"), &config),
            GroupItemValue::Nothing
        ));
    }
}
