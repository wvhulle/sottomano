use corredo::popovers::{CascadeDirection, CascadeOrientation, PopupGrowth};
use iced::Element;
use sottomano_widgets::{
    group_menus::{
        cascading_layout::popup_growth::growth_away_from,
        group_menu_state::GroupMenuTopic,
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
    },
    hover_cards::{HoverTarget, hover_card_surfaces::HoverContent},
    indicator_groups::{
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
        indicator_group_interaction::IndicatorGroupInteraction,
        tray_group::TrayMenuLook,
    },
    widgets::{typing_under_cards::CardBackgroundFade, window_icon::WindowAssets},
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn hover_content(
        &self,
        target: &HoverTarget,
    ) -> Option<HoverContent<'_, ShellInteraction>> {
        if let Some(GroupMenuCascadeSurfaces {
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
    ) -> Option<GroupMenuCascadeSurfaces<'_, ShellInteraction>> {
        match target {
            HoverTarget::GroupMenu(topic) => self.group_menu_surfaces(*topic),
            HoverTarget::Indicator(indicator_key)
                if indicator_key.group
                    == IndicatorGroupIdentity::sole(IndicatorGroupFamily::Tray) =>
            {
                self.tray_menu_surfaces(&indicator_key.name)
            }
            _ => None,
        }
    }

    fn tray_menu_surfaces(
        &self,
        item_name: &str,
    ) -> Option<GroupMenuCascadeSurfaces<'_, ShellInteraction>> {
        let group_menus = &self.indicator_groups.group_menus;
        let surfaces = self.indicator_groups.tray_group.menu_surfaces_named(
            item_name,
            &self.config,
            TrayMenuLook {
                appearance: group_menus.layer.appearance(),
                presentation: group_menus.presentation,
                orientation: self.group_menu_orientation(),
            },
        )?;
        let routed =
            |tray_message| ShellInteraction::Group(IndicatorGroupInteraction::Tray(tray_message));
        Some(GroupMenuCascadeSurfaces {
            primary: surfaces.primary.map(routed),
            satellites: surfaces
                .satellites
                .into_iter()
                .map(|satellite| satellite.map(routed))
                .collect(),
        })
    }

    fn group_menu_surfaces(
        &self,
        topic: GroupMenuTopic,
    ) -> Option<GroupMenuCascadeSurfaces<'_, ShellInteraction>> {
        let group_menu = self.group_menu_of(topic, self.hovered_output());
        let inspected_sections = self
            .indicator_groups
            .group_menus
            .inspection
            .inspected_indicator(topic)
            .map(|indicator_key| group_menu.indicator_hover_card(indicator_key, self.clock))
            .unwrap_or_default();
        let satellite_fade_level = self.hover_card_output().map_or(0.0, |output| {
            self.card_background_fade(output).fade_level_anywhere()
        });
        group_menu
            .surfaces(
                &self.config,
                &self
                    .indicator_groups
                    .group_menus
                    .layer
                    .appearance()
                    .with_background_faded(satellite_fade_level),
                &InspectionCard {
                    sections: &inspected_sections,
                    card_appearance: self
                        .indicator_groups
                        .overlay_settings_group
                        .layer
                        .appearance()
                        .with_background_faded(satellite_fade_level),
                    assets: self.window_assets(),
                    clock: self.clock,
                },
                self.group_menu_orientation(),
            )
            .map(|surfaces| GroupMenuCascadeSurfaces {
                primary: surfaces.primary.map(ShellInteraction::from),
                satellites: surfaces
                    .satellites
                    .into_iter()
                    .map(|satellite| satellite.map(ShellInteraction::from))
                    .collect(),
            })
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
