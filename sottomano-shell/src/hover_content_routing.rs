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
        group_menu_state::GroupMenuTopic,
        group_menu_view::InspectionCard,
        menu_rows::menu_row::{MenuAppearance, configured_nested_menu},
    },
    hover_cards::{HoverTarget, hover_card_body::Section, hover_card_surfaces::HoverContent},
    icon_catalog::window_icons::WindowAssets,
    indicator_groups::{
        indicator_group_content::GroupEvent,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
        indicator_group_interaction::IndicatorGroupInteraction,
        tray_group::TrayMessage,
    },
    widgets::typing_under_cards::CardBackgroundFade,
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
        match self.interactive_panel(target) {
            Some(panel) => Some(HoverContent::Interactive { panel }),
            None => self
                .described_hover_card(target)
                .map(HoverContent::Described),
        }
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

    fn cascading_menu_surfaces(
        &self,
        target: &HoverTarget,
    ) -> Option<NestedMenuSurfaces<'_, ShellInteraction>> {
        let groups = &self.indicator_groups;
        match target {
            HoverTarget::GroupMenu(GroupMenuTopic::WindowMenuBar) => self.cascading_menu(
                groups.window_menu_bar.menu_entries(&self.config),
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
                        &self.desktop_entries,
                        &self.config,
                        self.hovered_output(),
                        &app_card_view,
                    ),
                    &groups.app_launcher_menu.menu_state,
                    |menu_action| GroupEvent::from(AppLauncherMessage::Menu(menu_action)),
                )
            }
            HoverTarget::GroupMenu(topic) => self.group_menu_surfaces(*topic),
            HoverTarget::Indicator(indicator_key)
                if indicator_key.group
                    == IndicatorGroupIdentity::sole(IndicatorGroupFamily::Tray) =>
            {
                self.cascading_menu(
                    groups
                        .tray_group
                        .menu_entries_named(&indicator_key.name, &self.config),
                    &groups.tray_group.menu_state,
                    TrayMessage::Menu,
                )
            }
            _ => None,
        }
    }

    fn cascading_menu<'a, SubmenuKey, Message>(
        &self,
        entries: Vec<MenuEntry<'a, SubmenuKey, Message>>,
        menu_state: &NestedMenuState<SubmenuKey>,
        on_action: impl Fn(NestedMenuAction<SubmenuKey>) -> Message + 'a,
    ) -> Option<NestedMenuSurfaces<'a, ShellInteraction>>
    where
        SubmenuKey: Clone + PartialEq + 'a,
        Message: Clone + 'a,
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
        let satellite_fade_level = self.hover_card_output().map_or(0.0, |output| {
            self.card_background_fade(output).fade_level_anywhere()
        });
        MenuAppearance {
            list_appearance: group_menus
                .layer
                .appearance()
                .with_background_faded(satellite_fade_level),
            beside_appearance: self
                .indicator_groups
                .overlay_settings_group
                .layer
                .appearance()
                .with_background_faded(satellite_fade_level),
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
        let group_menu = self.group_menu_of(topic);
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

    fn interactive_panel(&self, target: &HoverTarget) -> Option<Element<'_, ShellInteraction>> {
        let HoverTarget::SessionMenu = target else {
            return None;
        };
        Some(
            self.indicator_groups
                .session_power_group
                .menu_panel(
                    &self.config,
                    self.indicator_groups
                        .overlay_settings_group
                        .layer
                        .appearance()
                        .frame,
                    self.indicator_groups.idle_pause_groups.timeouts(),
                )
                .map(|session_message| {
                    ShellInteraction::Group(IndicatorGroupInteraction::Session(session_message))
                }),
        )
    }

    #[must_use]
    pub fn window_assets(&self) -> WindowAssets<'_> {
        WindowAssets {
            entries: &self.desktop_entries,
            favicons: &self.favicons,
            controller_artwork: &self.controller_artwork,
        }
    }

    pub(crate) fn card_background_fade(&self, output: &str) -> CardBackgroundFade {
        self.typing_under_cards.background_fade_on(
            output,
            &self.compositor,
            self.clock.instant,
            &self.config.overlay_surfaces,
        )
    }

    pub(crate) fn hover_card_layer<'a>(&'a self, output: &str) -> Element<'a, ShellInteraction> {
        self.hover_card.view(
            output,
            &self.config,
            self.indicator_groups
                .hover_card_surface_style(self.card_background_fade(output)),
            self.window_assets(),
            |target: &HoverTarget| self.hover_content(target),
            self.clock,
        )
    }
}
