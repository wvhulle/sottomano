use corredo::popovers::{
    CascadeDirection, CascadeOrientation, MenuEntry, NestedMenuAction, NestedMenuState,
    NestedMenuSurfaces, PopupGrowth,
};
use iced::Element;
use sottomano_widgets::{
    cascading_menus::{
        app_launcher_menu::AppLauncherMessage, window_menu_bar::WindowMenuBarInteraction,
    },
    group_menus::{
        cascading_layout::popup_growth::growth_away_from,
        group_menu_state::{GroupMenuTopic, MenuTopicPresence},
        group_menu_view::InspectionCard,
        menu_rows::menu_row::{MenuAppearance, configured_nested_menu},
    },
    hover_cards::{HoverTarget, hover_card_body::Section, hover_card_surfaces::HoverContent},
    icon_catalog::window_icons::WindowAssets,
    indicator_groups::{
        indicator_group_content::GroupEvent,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
        session_power_group::SessionMessage,
        tray_group::TrayMessage,
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn hover_content(
        &self,
        target: &HoverTarget,
    ) -> Option<HoverContent<'_, ShellInteraction>> {
        if let Some(NestedMenuSurfaces {
            primary,
            satellites,
        }) = self.cascading_menu_surfaces(target)
        {
            return Some(HoverContent::Cascading {
                primary,
                satellites,
                orientation: self.group_menu_orientation(),
            });
        }
        self.described_hover_card(target)
            .map(HoverContent::Described)
    }

    fn group_menu_orientation(&self) -> CascadeOrientation {
        let Some((placement, anchor)) = self
            .hover_card
            .transition
            .placement()
            .zip(self.hover_card.shown_anchor())
        else {
            return CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PopupGrowth::Downward,
            };
        };
        let output_width = self
            .outputs
            .get(&placement.output)
            .map_or(0.0, |output| output.logical_width as f32);
        CascadeOrientation::toward_room(anchor, growth_away_from(placement.position), output_width)
    }

    pub(crate) fn hovered_output(&self) -> &str {
        self.hover_card
            .transition
            .placement()
            .map_or("", |placement| placement.output.as_str())
    }

    pub(crate) fn open_group_menu(&self) -> Option<GroupMenuTopic> {
        match self.hover_card.target() {
            Some(HoverTarget::GroupMenu(topic)) => Some(*topic),
            _ => None,
        }
    }

    pub(crate) fn window_menu_bar_presence(&self) -> MenuTopicPresence {
        match self.hover_card.target() {
            Some(HoverTarget::WindowTopMenu(_)) => MenuTopicPresence::RecentlyOpen,
            _ => self
                .indicator_groups
                .group_menus
                .topic_presence(GroupMenuTopic::WindowMenuBar),
        }
    }

    fn cascading_menu_surfaces(
        &self,
        target: &HoverTarget,
    ) -> Option<NestedMenuSurfaces<'_, ShellInteraction>> {
        let groups = &self.indicator_groups;
        match target {
            HoverTarget::GroupMenu(GroupMenuTopic::WindowMenuBar) => self.cascading_menu(
                groups
                    .window_menu_bar
                    .menu_entries(&self.config, &self.icon_catalog),
                &groups.window_menu_bar.menu_state,
                WindowMenuBarInteraction::Menu,
            ),
            HoverTarget::WindowTopMenu(top_menu_id) => self.cascading_menu(
                groups.window_menu_bar.top_menu_entries(
                    *top_menu_id,
                    &self.config,
                    &self.icon_catalog,
                ),
                &groups.window_menu_bar.menu_state,
                WindowMenuBarInteraction::Menu,
            ),
            HoverTarget::GroupMenu(GroupMenuTopic::Applications) => {
                let app_card_view = |sections: Vec<Section>| {
                    self.inspection_card(&sections)
                        .view(&self.config)
                        .map(|card| card.map(GroupEvent::Popup))
                };
                self.cascading_menu(
                    groups.app_launcher_menu.menu_entries(
                        &self.icon_catalog,
                        &self.config,
                        self.hovered_output(),
                        &app_card_view,
                    ),
                    &groups.app_launcher_menu.menu_state,
                    |menu_action| GroupEvent::from(AppLauncherMessage::Menu(menu_action)),
                )
            }
            HoverTarget::GroupMenu(GroupMenuTopic::SessionPower) => self.cascading_menu(
                groups
                    .session_power_group
                    .menu_entries(&self.config, groups.idle_pause_groups.idle_chain()),
                &groups.session_power_group.menu_state,
                SessionMessage::Menu,
            ),
            HoverTarget::GroupMenu(topic) => self.group_menu_surfaces(*topic),
            HoverTarget::Indicator(indicator_key)
                if indicator_key.group
                    == IndicatorGroupIdentity::sole(IndicatorGroupFamily::Tray) =>
            {
                self.cascading_menu(
                    groups.tray_group.menu_entries_named(
                        &indicator_key.name,
                        &self.config,
                        &self.icon_catalog,
                    ),
                    &groups.tray_group.menu_state,
                    TrayMessage::Menu,
                )
            }
            _ => None,
        }
    }

    fn cascading_menu<'view, SubmenuKey, Message>(
        &self,
        entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
        menu_state: &NestedMenuState<SubmenuKey>,
        on_action: impl Fn(NestedMenuAction<SubmenuKey>) -> Message + 'view,
    ) -> Option<NestedMenuSurfaces<'view, ShellInteraction>>
    where
        SubmenuKey: Clone + PartialEq + 'view,
        Message: Clone + 'view,
        GroupEvent: From<Message>,
    {
        configured_nested_menu(
            entries,
            menu_state,
            on_action,
            &self.config,
            self.menu_appearance(),
        )
        .map(|surfaces| surfaces.map(|message| ShellInteraction::from(GroupEvent::from(message))))
    }

    fn menu_appearance(&self) -> MenuAppearance {
        let group_menus = &self.indicator_groups.group_menus;
        MenuAppearance {
            list_appearance: group_menus.layer.appearance(),
            beside_appearance: self
                .indicator_groups
                .overlay_settings_group
                .layer
                .appearance(),
            presentation: group_menus.presentation,
            orientation: self.group_menu_orientation(),
        }
    }

    fn inspection_card<'sections>(
        &self,
        sections: &'sections [Section],
    ) -> InspectionCard<'sections, '_> {
        InspectionCard {
            sections,
            card_appearance: self.menu_appearance().beside_appearance,
            assets: self.window_assets(),
            clock: self.clock,
        }
    }

    fn group_menu_surfaces(
        &self,
        topic: GroupMenuTopic,
    ) -> Option<NestedMenuSurfaces<'_, ShellInteraction>> {
        let group_menu = self.topic_group_menu(topic);
        let inspected_sections = self
            .indicator_groups
            .group_menus
            .inspection
            .inspected_indicator(topic)
            .map(|indicator_key| group_menu.indicator_hover_card(indicator_key, self.clock))
            .unwrap_or_default();
        let appearance = self.menu_appearance();
        group_menu
            .surfaces(
                &self.config,
                &appearance.list_appearance,
                &self.inspection_card(&inspected_sections),
                appearance.orientation,
            )
            .map(|surfaces| surfaces.map(ShellInteraction::from))
    }

    #[must_use]
    pub fn window_assets(&self) -> WindowAssets<'_> {
        WindowAssets {
            icon_catalog: &self.icon_catalog,
            desktop_entries: &self.desktop_entries,
            favicons: &self.favicons,
            controller_artwork: &self.controller_artwork,
        }
    }

    pub(crate) fn hover_card_layer<'view>(
        &'view self,
        output: &str,
    ) -> Element<'view, ShellInteraction> {
        self.hover_card.view(
            output,
            &self.config,
            self.indicator_groups.hover_card_surface_style(),
            self.window_assets(),
            |target: &HoverTarget| self.hover_content(target),
            self.clock,
        )
    }
}
