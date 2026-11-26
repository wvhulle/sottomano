use corredo::{
    popovers::{
        CascadeOrientation, MenuEntry, MenuRowEmphasis, MenuSubmenu, NestedMenuSurfaces,
        submenu_header_emphasis,
    },
    transitions::ExpansionState,
};
use sottomano_toml_config::Config;

use super::{GroupMenuView, InspectionCard};
use crate::{
    group_indicators::{
        indicator_group_content::{GroupEvent, IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupIdentity,
    },
    group_menus::{
        group_menu_state::{AlertPromotion, GroupMenuPin},
        menu_rows::{
            group_menu_entries::{
                GroupMenuEntry, GroupMenuRowContext, group_header, indicator_entry,
                one_entry_per_identity,
            },
            menu_row::{MenuAppearance, configured_nested_menu},
        },
    },
    hover_cards::PanelPinOrigin,
    widgets::shell_layer_appearance::LayerAppearance,
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
        let group_entries = groups
            .into_iter()
            .filter(|member| {
                let group_entry = GroupMenuPin::Group(member.group_identity.clone());
                group_menus.panel_pin(messenger.topic, group_entry, AlertPromotion::Unpromoted)
                    == PanelPinOrigin::Unpinned
            })
            .map(IndicatorGroupContent::menu_entry)
            .collect();
        let presentation = group_menus.presentation;
        let row_context = GroupMenuRowContext {
            config,
            menu: &messenger,
            group_menus,
            clock: card.clock,
        };
        let menu_entries = group_menu_entries(&row_context, group_entries, card);
        configured_nested_menu(
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
        )
    }
}

fn group_menu_entries<'view>(
    row_context: &GroupMenuRowContext<'_>,
    group_entries: Vec<GroupMenuEntry<'view>>,
    card: &InspectionCard<'_, 'view>,
) -> Vec<MenuEntry<'view, IndicatorGroupIdentity, GroupEvent>> {
    let topic = row_context.menu.topic;
    let open_groups = row_context.group_menus.inspection.open_groups(topic);
    let inspected_indicator = row_context
        .group_menus
        .inspection
        .inspected_indicator(topic);
    let layout = row_context.group_menus.presentation.layout;
    one_entry_per_identity(group_entries)
        .into_iter()
        .map(|entry| {
            let expansion_state = if open_groups.contains(&entry.group_identity) {
                ExpansionState::Expanded
            } else {
                ExpansionState::Collapsed
            };
            let header_emphasis = match &entry.listing {
                IndicatorListing::Listed(listed_indicators) if listed_indicators.is_empty() => {
                    MenuRowEmphasis::Unavailable
                }
                IndicatorListing::Listed(_) | IndicatorListing::Incomplete { .. } => {
                    submenu_header_emphasis(layout, expansion_state)
                }
            };
            let header = group_header(row_context, &entry, header_emphasis);
            let indicators = entry.listing.iter().map(|indicator| {
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
