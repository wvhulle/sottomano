use corredo::popovers::{MenuEntry, NestedMenuOpenTrigger};
use iced::Element;
use sottomano_desktop_sources::desktop_windows_apps::application_menu_nodes::MenuNode;
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use super::{WindowMenuBar, WindowMenuBarInteraction, menu_bar_switch::WindowMenuBarLayout};
use crate::{
    cascading_menus::menu_node_entries::menu_node_entries,
    group_menus::group_menu_view::panel_menu_handle,
    hover_cards::{HoverTarget, MenuHandleIndicator},
    icon_catalog::drawable_icon_catalog::{DrawableIconSources, IconCatalog},
    indicator_groups::indicator_group_content::GroupEvent,
    widgets::text::panel_text,
};

impl WindowMenuBar {
    #[must_use]
    pub fn shows_inline_top_menus(&self) -> bool {
        self.shows_top_menus() && self.window_menu_bar_layout == WindowMenuBarLayout::InlineTopMenus
    }

    pub(super) fn top_menu_children(
        &self,
        top_menu: &MenuNode,
        config: &Config,
        icon_catalog: &IconCatalog,
    ) -> Vec<MenuEntry<'static, i32, WindowMenuBarInteraction>> {
        menu_node_entries(
            &top_menu.children,
            config,
            DrawableIconSources {
                icon_catalog,
                pixmap_handles: &self.pixmap_handles,
            },
            &WindowMenuBarInteraction::ItemPressed,
        )
    }

    #[must_use]
    pub fn top_menu_entries(
        &self,
        top_menu_id: i32,
        config: &Config,
        icon_catalog: &IconCatalog,
    ) -> Vec<MenuEntry<'static, i32, WindowMenuBarInteraction>> {
        self.top_menus()
            .iter()
            .filter(|top_menu| top_menu.id == top_menu_id)
            .flat_map(|top_menu| self.top_menu_children(top_menu, config, icon_catalog))
            .collect()
    }

    pub fn inline_handle_views<'a>(
        &'a self,
        config: &'a Config,
        panel_position: Position,
        open_trigger: NestedMenuOpenTrigger,
        handle_indicator: impl Fn(&HoverTarget) -> MenuHandleIndicator,
    ) -> Vec<Element<'a, GroupEvent>> {
        self.top_menus()
            .iter()
            .filter_map(|top_menu| {
                let top_menu_label = top_menu.label.as_deref()?;
                let hover_target = HoverTarget::WindowTopMenu(top_menu.id);
                let top_menu_indicator = handle_indicator(&hover_target);
                Some(panel_menu_handle(
                    open_trigger,
                    panel_text(&config.theme.font, top_menu_label).into(),
                    hover_target,
                    Severity::Normal,
                    top_menu_indicator,
                    panel_position,
                ))
            })
            .collect()
    }
}
