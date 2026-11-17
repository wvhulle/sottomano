use corredo::{
    orientation::Axis,
    popovers::{
        CascadeOrientation, MenuEntry, MenuRowEmphasis, MenuSubmenu, NestedMenuAction, nested_menu,
        submenu_header_emphasis,
    },
    selection_scroll::{ScrollEdgeFade, ScrollPacing},
    transitions::ExpansionState,
};
use iced::widget::Column;
use sottomano_toml_config::Config;

use super::{GroupMenuCascadeSurfaces, GroupMenuView, InspectionCard, MenuSections};
use crate::{
    group_menus::{
        cascading_layout::menu_satellite_regions::{SatelliteBlur, satellite_with_regions},
        group_menu_state::{AlertPromotion, GroupMenuPin},
        menu_rows::{
            group_menu_group_rows::{
                GroupMenuGroupRow, GroupMenuRowContext, group_header, indicator_entry,
                one_row_per_identity,
            },
            menu_row::{MenuFrames, menu_metrics, menu_style, phosphor_caret},
            quantity_table::quantity_table,
        },
    },
    hover_cards::PanelPinOrigin,
    indicator_groups::{
        indicator_group_content::GroupEvent, indicator_group_identity::IndicatorGroupIdentity,
    },
    widgets::{configured_scroll::configured_scroll, shell_layer_appearance::LayerAppearance},
};

impl<'a> GroupMenuView<'a> {
    #[must_use]
    pub fn surfaces(
        self,
        config: &'a Config,
        appearance: &LayerAppearance,
        card: &InspectionCard<'_, 'a>,
        orientation: CascadeOrientation,
    ) -> Option<GroupMenuCascadeSurfaces<'a, GroupEvent>> {
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
        let style = menu_style(
            config,
            MenuFrames {
                list_frame,
                beside_frame: card.card_appearance.frame,
            },
            presentation.submenu_connector,
        );
        let fade_extent_px = style.fade_extent_px;
        let scrolled_px = group_menus.inspection.submenus.scrolled_px(0);
        let menu_action = messenger.menu_action();
        let satellite_blur = SatelliteBlur {
            item_list: appearance.backdrop_blur,
            beside: card.card_appearance.backdrop_blur,
        };
        let cascade = nested_menu(
            menu_entries,
            &group_menus.inspection.submenus,
            presentation,
            messenger.menu_action(),
        )
        .metrics(menu_metrics(config))
        .orientation(orientation)
        .caret(phosphor_caret(config))
        .style(style)
        .wheel_scrolling(
            config.scrolling.wheel_line_px,
            ScrollPacing::Eased(config.scrolling.wheel_easing),
        )
        .surfaces()
        .map(|surfaces| GroupMenuCascadeSurfaces {
            primary: surfaces.primary,
            satellites: surfaces
                .satellites
                .into_iter()
                .map(|satellite| satellite_with_regions(satellite, satellite_blur))
                .collect(),
        });
        let Some(table) = table else {
            return cascade;
        };
        let Some(cascade) = cascade else {
            return Some(GroupMenuCascadeSurfaces {
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
        Some(GroupMenuCascadeSurfaces {
            primary: Column::with_children([table, cascade.primary]).into(),
            satellites: cascade.satellites,
        })
    }
}

fn group_menu_entries<'a>(
    row_context: &GroupMenuRowContext<'_>,
    group_rows: Vec<GroupMenuGroupRow<'a>>,
    card: &InspectionCard<'_, 'a>,
) -> Vec<MenuEntry<'a, IndicatorGroupIdentity, GroupEvent>> {
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
