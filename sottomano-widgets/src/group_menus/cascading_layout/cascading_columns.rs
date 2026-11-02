use std::mem::take;

use iced::Element;
use sottomano_config::Config;

use crate::{
    group_menus::{
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, group_row, item_menu},
            menu_row::{MenuRowLook, menu_row_height},
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

pub(crate) fn cascading_columns<
    'a,
    M: Clone + 'a,
    Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a,
>(
    mut entries: Vec<GroupMenuGroupRow<'a, M>>,
    layout: &GroupMenuRowContext<'_, 'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Option<GroupMenuCascadeSurfaces<'a, Outer>> {
    if entries.is_empty() {
        return None;
    }
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
            highlighted: open_position == Some(position),
            corners: group_corners.radius_at(position),
        };
        group_row(layout, entry, look)
    });
    Some(GroupMenuCascadeSurfaces {
        primary: Element::from(uniform_width_column(group_rows)).map(Outer::from),
        satellites: open_group
            .map(|open| open_group_satellites(layout, open, card))
            .unwrap_or_default(),
    })
}

fn open_group_satellites<
    'a,
    M: Clone + 'a,
    Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a,
>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    open: OpenGroup<'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Vec<Element<'a, Outer>> {
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
    let item_count = items.len();
    let item_corners = layout.cascade_satellite_panels.frame_corners;
    let is_inspected = |item: &GroupMenuItemRow<'a, M>| {
        inspected_target.is_some_and(|target| item.inspected_target == *target)
    };
    let item_list =
        item_menu(layout, &group_identity, items, &is_inspected, item_corners).map(Outer::from);
    let item_satellite = layout.cascade_satellite_panels.beside_row(
        layout.config,
        item_list,
        cascade_offset(layout.config, open_position),
    );
    let card_satellite = card_position.and_then(|position| {
        card.view(layout.config).map(|card_view| {
            let card_row_top = layout.cascade_satellite_panels.item_row_top(
                layout.config,
                open_position,
                position,
                item_count,
            );
            layout.cascade_satellite_panels.beside_row(
                layout.config,
                card_view.map(Outer::from),
                card_row_top,
            )
        })
    });
    [item_satellite].into_iter().chain(card_satellite).collect()
}

pub(crate) fn cascade_offset(config: &Config, rows_above: usize) -> f32 {
    menu_row_height(config) * rows_above as f32
}
