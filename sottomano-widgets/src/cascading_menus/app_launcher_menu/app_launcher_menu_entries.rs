use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuSubmenu},
    theming::{primary_color, text_color},
};
use iced::Element;
use sottomano_desktop_sources::{
    desktop_windows_apps::desktop_entry_app_catalog::{AppCategory, LaunchableApp},
    theme_icons::icon_requests::app_icon_request,
};
use sottomano_toml_config::Config;

use super::{AppLauncherMenu, AppLauncherMessage, AppLauncherSubmenu, InspectedApp};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::indicator_group_content::GroupEvent,
    hover_cards::hover_card_body::Section,
    icon_catalog::drawable_icon_catalog::IconCatalog,
    widgets::glyph_button,
};

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

impl AppLauncherMenu {
    pub(super) fn jump_button<'view>(
        &self,
        app: &LaunchableApp,
        config: &Config,
    ) -> Option<Element<'view, GroupEvent>> {
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

    fn app_entry<'view>(
        &self,
        app: &LaunchableApp,
        icon_catalog: &IconCatalog,
        config: &Config,
        card_view: &dyn Fn(Vec<Section>) -> Option<Element<'view, GroupEvent>>,
    ) -> MenuEntry<'view, AppLauncherSubmenu, GroupEvent> {
        let icon_size_px = config.theme.font.icon_size_px;
        let content = MenuRowContent::new(app.name.clone());
        let content = match icon_catalog.drawable_icon(&app_icon_request(&app.app_id)) {
            Some(drawable_icon) => content.icon(drawable_icon.view(icon_size_px, 1.0)),
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

    fn submenu_header<'view>(
        title: &'static str,
        glyph: Phosphor,
        config: &Config,
    ) -> MenuRowContent<'view, GroupEvent> {
        MenuRowContent::new(title)
            .icon(IconGlyph::from(glyph).view(config.theme.font.icon_size_px, text_color))
    }

    #[must_use]
    pub fn menu_entries<'view>(
        &self,
        icon_catalog: &IconCatalog,
        config: &Config,
        output: &str,
        card_view: &dyn Fn(Vec<Section>) -> Option<Element<'view, GroupEvent>>,
    ) -> Vec<MenuEntry<'view, AppLauncherSubmenu, GroupEvent>> {
        let frequent_entries: Vec<_> = self
            .frequent_apps_on(output)
            .map(|app| self.app_entry(app, icon_catalog, config, card_view))
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
                        .map(|app| self.app_entry(app, icon_catalog, config, card_view)),
                )
                .into()
            });
        frequent_submenu
            .into_iter()
            .chain(category_submenus)
            .collect()
    }
}
