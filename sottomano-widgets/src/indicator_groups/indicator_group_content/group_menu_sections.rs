use std::collections::BTreeMap;

use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position};

use super::{GroupEvent, IndicatorGroupContent, group_panel_view::fixed_view};
use crate::{
    clock::Clock,
    group_indicators::aligned_quantity::IndicatorAlignment,
    group_menus::{
        group_menu_state::{GroupMenuState, GroupMenuTopic, IndicatorPin},
        group_menu_view::MenuSections,
        menu_rows::{
            group_menu_group_rows::GroupMenuGroupRow, quantity_table::QuantityTableColumn,
        },
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
        self.indicators
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

    pub(crate) fn extend_menu_sections(
        self,
        config: &'view Config,
        clock: Clock,
        _group_menus: &GroupMenuState,
        _menu: GroupMenuTopic,
        mut sections: MenuSections<'view>,
    ) -> MenuSections<'view> {
        let IndicatorGroupContent {
            group_identity,
            group_title,
            headline_indicator,
            indicators,
            ..
        } = self;
        let (item_rows, aligned_indicators): (Vec<_>, Vec<_>) = indicators
            .into_iter()
            .partition(|indicator| indicator.alignment == IndicatorAlignment::Unaligned);
        let aligned = aligned_indicators
            .iter()
            .fold(BTreeMap::new(), |mut aligned, indicator| {
                if let IndicatorAlignment::Aligned(quantity) = indicator.alignment {
                    aligned.insert(quantity, indicator.table_cell_view(config, clock));
                }
                aligned
            });
        if !aligned.is_empty() {
            sections.table_columns.push(QuantityTableColumn {
                group_title: group_title.clone(),
                aligned,
            });
        }
        if !item_rows.is_empty() {
            sections.group_rows.push(GroupMenuGroupRow {
                group_identity,
                group_title,
                headline_indicator,
                indicators: item_rows,
            });
        }
        sections
    }
}
