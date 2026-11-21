use corredo::{
    orientation::Axis,
    popovers::{
        CascadeOrientation, MenuEntry, MenuRowEmphasis, MenuSubmenu, NestedMenuAction,
        NestedMenuSurfaces, submenu_header_emphasis,
    },
    selection_scroll::ScrollEdgeFade,
    transitions::ExpansionState,
};
use iced::widget::Column;
use sottomano_toml_config::Config;

use super::{GroupMenuView, InspectionCard, MenuSections};
use crate::{
    group_menus::{
        group_menu_state::{AlertPromotion, GroupMenuPin},
        menu_rows::{
            group_menu_group_rows::{
                GroupMenuGroupRow, GroupMenuRowContext, group_header, indicator_entry,
                one_row_per_identity,
            },
            menu_row::{MenuAppearance, configured_nested_menu, menu_metrics},
            quantity_table::quantity_table,
        },
    },
    hover_cards::PanelPinOrigin,
    indicator_groups::{
        indicator_group_content::GroupEvent, indicator_group_identity::IndicatorGroupIdentity,
    },
    widgets::{configured_scroll::configured_scroll, shell_layer_appearance::LayerAppearance},
};

impl<'view> GroupMenuView<'view> {
    #[must_use]
    pub fn surfaces(
        self,
        config: &'view Config,
        appearance: &LayerAppearance,
        card: &InspectionCard<'_, 'view>,
        orientation: CascadeOrientation,
    ) -> Option<NestedMenuSurfaces<'view, GroupEvent>> {
        let GroupMenuView {
            group_menus,
            messenger,
            groups,
            ..
        } = self;
        let MenuSections {
            table_columns,
            group_rows,
        } = groups
            .into_iter()
            .filter(|member| {
                let group_entry = GroupMenuPin::Group(member.group_identity.clone());
                group_menus.panel_pin(messenger.topic, group_entry, AlertPromotion::Unpromoted)
                    == PanelPinOrigin::Unpinned
            })
            .fold(MenuSections::empty(), |sections, member| {
                member.extend_menu_sections(
                    config,
                    card.clock,
                    group_menus,
                    messenger.topic,
                    sections,
                )
            });
        let table = quantity_table(table_columns, config);
        let presentation = group_menus.presentation;
        let row_context = GroupMenuRowContext {
            config,
            menu: &messenger,
            group_menus,
            clock: card.clock,
        };
        let menu_entries = group_menu_entries(&row_context, group_rows, card);
        let list_frame = appearance.frame;
        let fade_extent_px =
            config.group_menus.scroll_fade_rows * menu_metrics(config).row_height_px;
        let scrolled_px = group_menus.inspection.submenus.scrolled_px(0);
        let menu_action = messenger.menu_action();
        let cascade = configured_nested_menu(
            menu_entries,
            &group_menus.inspection.submenus,
            messenger.menu_action(),
            config,
            MenuAppearance {
                list_appearance: *appearance,
                beside_appearance: card.card_appearance,
                presentation,
                orientation,
            },
        );
        let Some(table) = table else {
            return cascade;
        };
        let Some(cascade) = cascade else {
            return Some(NestedMenuSurfaces {
                primary: configured_scroll(table, Axis::Vertical, config)
                    .edge_fade(ScrollEdgeFade::within_frame(list_frame, fade_extent_px))
                    .controlled(scrolled_px, move |scrolled_px| {
                        menu_action(NestedMenuAction::Scrolled {
                            depth: 0,
                            scrolled_px,
                        })
                    })
                    .into(),
                satellites: Vec::new(),
            });
        };
        Some(NestedMenuSurfaces {
            primary: Column::with_children([table, cascade.primary]).into(),
            satellites: cascade.satellites,
        })
    }
}

fn group_menu_entries<'view>(
    row_context: &GroupMenuRowContext<'_>,
    group_rows: Vec<GroupMenuGroupRow<'view>>,
    card: &InspectionCard<'_, 'view>,
) -> Vec<MenuEntry<'view, IndicatorGroupIdentity, GroupEvent>> {
    let topic = row_context.menu.topic;
    let open_groups = row_context.group_menus.inspection.open_groups(topic);
    let inspected_indicator = row_context
        .group_menus
        .inspection
        .inspected_indicator(topic);
    let layout = row_context.group_menus.presentation.layout;
    one_row_per_identity(group_rows)
        .into_iter()
        .map(|entry| {
            let expansion_state = if open_groups.contains(&entry.group_identity) {
                ExpansionState::Expanded
            } else {
                ExpansionState::Collapsed
            };
            let header = group_header(
                row_context,
                &entry.group_identity,
                &entry.group_title,
                &entry.headline_indicator,
                submenu_header_emphasis(layout, expansion_state),
            );
            let indicators = entry.indicators.iter().map(|indicator| {
                let item = indicator_entry(row_context, &entry.group_identity, indicator);
                let inspected_card =
                    match indicator.row_emphasis(&entry.group_identity, inspected_indicator) {
                        MenuRowEmphasis::Selected => card.view(row_context.config),
                        MenuRowEmphasis::Plain
                        | MenuRowEmphasis::Opened
                        | MenuRowEmphasis::Unavailable => None,
                    };
                match inspected_card {
                    Some(card_view) => item.beside(card_view.map(GroupEvent::Popup)),
                    None => item,
                }
                .into()
            });
            MenuSubmenu::new(entry.group_identity.clone(), header, indicators).into()
        })
        .collect()
}
