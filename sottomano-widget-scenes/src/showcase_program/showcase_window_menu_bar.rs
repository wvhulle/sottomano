use corredo::popovers::CascadeOrientation;
use iced::Element;
use sottomano_widgets::{
    cascading_menus::window_menu_bar::WindowMenuBarInteraction,
    group_indicators::indicator_group_content::GroupEvent,
    group_menus::{
        cascading_layout::popup_growth::growth_away_from,
        menu_rows::menu_row::{MenuAppearance, configured_nested_menu},
    },
    hover_cards::hover_card_surfaces::HoverContent,
};

use super::ShowcaseState;
use crate::{
    showcase_message::ShowcaseInteraction,
    showcased_group::{SHOWCASE_OUTPUT, ShowcasedGroup},
};

impl ShowcaseState {
    pub(super) fn window_top_menu_handles(&self) -> Option<Vec<Element<'_, ShowcaseInteraction>>> {
        let ShowcasedGroup::WindowMenuBar(menu_bar) = &self.group else {
            return None;
        };
        Some(
            menu_bar
                .group
                .inline_handle_views(
                    &self.config,
                    self.group.position(),
                    self.group_menus.presentation.open_trigger,
                    |target| self.hover_card.handle_indicator(target, SHOWCASE_OUTPUT),
                )
                .into_iter()
                .map(|element| element.map(ShowcaseInteraction::from))
                .collect(),
        )
    }

    pub(super) fn window_top_menu_cascade(
        &self,
        top_menu_id: i32,
    ) -> Option<HoverContent<'_, ShowcaseInteraction>> {
        let ShowcasedGroup::WindowMenuBar(menu_bar) = &self.group else {
            return None;
        };
        let orientation = CascadeOrientation::toward_room(
            self.hover_card.shown_anchor().unwrap_or_default(),
            growth_away_from(self.group.position()),
            self.scene.backdrop.canvas().width,
        );
        let surfaces = configured_nested_menu(
            menu_bar
                .group
                .top_menu_entries(top_menu_id, &self.config, &menu_bar.icon_catalog),
            &menu_bar.group.menu_state,
            WindowMenuBarInteraction::Menu,
            &self.config,
            MenuAppearance {
                list_appearance: self.appearance.menus,
                beside_appearance: self.appearance.overlays,
                presentation: self.group_menus.presentation,
                orientation,
            },
        )?
        .map(|interaction| ShowcaseInteraction::from(GroupEvent::from(interaction)));
        Some(HoverContent::Cascading {
            primary: surfaces.primary,
            satellites: surfaces.satellites,
            orientation,
        })
    }
}
