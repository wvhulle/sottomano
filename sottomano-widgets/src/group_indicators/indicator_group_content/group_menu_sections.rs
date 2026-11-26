use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position};

use super::{GroupEvent, IndicatorGroupContent, group_panel_view::fixed_view};
use crate::{
    clock::Clock,
    group_menus::{
        group_menu_state::{GroupMenuState, GroupMenuTopic, IndicatorPin},
        menu_rows::group_menu_entries::GroupMenuEntry,
    },
    hover_cards::{HoverTarget, MenuHandleIndicator},
};

impl<'view> IndicatorGroupContent<'view> {
    pub(crate) fn panel_indicator_views(
        self,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
        config: &'view Config,
        panel_position: Position,
        clock: Clock,
    ) -> Vec<Element<'view, GroupEvent>> {
        let promotion_severity = config.group_menus.alert_promotion_severity;
        self.listing
            .iter()
            .filter_map(|indicator| {
                let IndicatorPin::Pinned(panel_pin) = group_menus.indicator_pin(
                    menu,
                    &self.group_identity,
                    indicator,
                    promotion_severity,
                ) else {
                    return None;
                };
                Some(fixed_view(
                    indicator,
                    HoverTarget::PinnedOnPanel(Box::new(panel_pin)),
                    indicator.shown_alert_severity().unwrap_or_default(),
                    MenuHandleIndicator::default(),
                    config,
                    panel_position,
                    clock,
                ))
            })
            .collect()
    }

    pub(crate) fn menu_entry(self) -> GroupMenuEntry<'view> {
        let IndicatorGroupContent {
            group_identity,
            group_title,
            headline_indicator,
            listing,
            ..
        } = self;
        GroupMenuEntry {
            group_identity,
            group_title,
            headline_indicator,
            listing,
        }
    }
}
