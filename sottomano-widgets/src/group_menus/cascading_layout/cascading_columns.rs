use std::mem::take;

use iced::Element;
use sottomano_config::Config;

use crate::{
    group_menus::{
        accordion_layout::menu_rows_viewport::menu_rows_viewport,
        cascading_layout::notched_panel::ResolvedPanelTop,
        group_menu_presentation::HoverCardPlacement,
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, group_row, item_menu},
            menu_row::{MenuRowEmphasis, MenuRowLook, menu_row_height},
        },
    },
    hover_cards::PopupMessage,
    widget_groups::{
        widget_group_content::{GroupEvent, GroupMenuItemRow},
        widget_group_identity::WidgetGroupIdentity,
    },
    widgets::{list_end_corners::ListEndCorners, uniform_width_column::uniform_width_column},
};

struct OpenGroup<'a, M> {
    position: usize,
    group_identity: WidgetGroupIdentity,
    items: Vec<GroupMenuItemRow<'a, M>>,
}

pub(crate) fn cascading_columns<'a, M: Clone + From<GroupEvent<M>> + From<PopupMessage> + 'a>(
    mut entries: Vec<GroupMenuGroupRow<'a, M>>,
    layout: &GroupMenuRowContext<'_, 'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Option<GroupMenuCascadeSurfaces<'a, M>> {
    if entries.is_empty() {
        return None;
    }
    let scrolled_px = layout.group_menus.scrolled_px(layout.menu.topic);
    let scroll_message = layout.menu.scroll_message();
    let open_identity = layout.group_menus.open_groups(layout.menu.topic).first();
    let open_group = open_identity
        .and_then(|identity| {
            entries
                .iter()
                .position(|entry| entry.group_identity == *identity)
        })
        .map(|position| OpenGroup {
            position,
            group_identity: entries[position].group_identity.clone(),
            items: take(&mut entries[position].items),
        });
    let open_position = open_group.as_ref().map(|open| open.position);
    let group_corners =
        ListEndCorners::new(entries.len(), layout.cascade_satellite_panels.frame_corners);
    let group_rows = entries.into_iter().enumerate().map(|(position, entry)| {
        let look = MenuRowLook {
            emphasis: MenuRowEmphasis::selected_when(open_position == Some(position)),
            corners: group_corners.radius_at(position),
        };
        group_row(layout, entry, look)
    });
    Some(GroupMenuCascadeSurfaces {
        primary: menu_rows_viewport(
            Element::from(uniform_width_column(group_rows)).map(M::from),
            scrolled_px,
            move |scrolled_px| M::from(scroll_message(scrolled_px)),
            layout
                .cascade_satellite_panels
                .rows_edge_fade(layout.config),
        ),
        satellites: open_group
            .map(|open| open_group_satellites(layout, open, card))
            .unwrap_or_default(),
    })
}

fn open_group_satellites<'a, M: Clone + From<GroupEvent<M>> + From<PopupMessage> + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    open: OpenGroup<'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Vec<Element<'a, M>> {
    let scrolled_px = layout.group_menus.scrolled_px(layout.menu.topic);
    let OpenGroup {
        position: open_position,
        group_identity,
        items,
    } = open;
    let inspected_target = layout.group_menus.inspected_target(layout.menu.topic);
    let card_position = inspected_target.and_then(|target| {
        items
            .iter()
            .position(|item| item.inspected_target == *target)
    });
    let item_corners = layout.cascade_satellite_panels.frame_corners;
    let is_inspected = |item: &GroupMenuItemRow<'a, M>| {
        inspected_target.is_some_and(|target| item.inspected_target == *target)
    };
    let inline_cards =
        layout.presentation.hover_card_placement == HoverCardPlacement::InlineWithRow;
    let inline_card = inline_cards
        .then(|| card_position.and_then(|_| card.view(layout.config)))
        .flatten()
        .map(|card_view| card_view.map(M::from));
    let item_list = item_menu(
        layout,
        &group_identity,
        items,
        &is_inspected,
        item_corners,
        inline_card,
    );
    let item_list_scrolled_px = layout.group_menus.item_list_scrolled_px(layout.menu.topic);
    let item_list_scroll_message = layout.menu.item_list_scroll_message();
    let item_list_viewport = menu_rows_viewport(
        item_list,
        item_list_scrolled_px,
        move |scrolled_px| M::from(item_list_scroll_message(scrolled_px)),
        layout
            .cascade_satellite_panels
            .rows_edge_fade(layout.config),
    );
    let item_list_top = ResolvedPanelTop::default();
    let item_satellite = layout.cascade_satellite_panels.item_list_beside_row(
        layout.config,
        item_list_viewport,
        cascade_offset(layout.config, open_position) - scrolled_px,
        item_list_top.clone(),
    );
    let card_satellite = card_position
        .filter(|_| !inline_cards)
        .and_then(|position| {
            card.view(layout.config).map(|card_view| {
                layout.cascade_satellite_panels.card_beside_item_row(
                    layout.config,
                    card_view.map(M::from),
                    item_list_top,
                    cascade_offset(layout.config, position) - item_list_scrolled_px,
                )
            })
        });
    [item_satellite].into_iter().chain(card_satellite).collect()
}

pub(crate) fn cascade_offset(config: &Config, rows_above: usize) -> f32 {
    menu_row_height(config) * rows_above as f32
}
