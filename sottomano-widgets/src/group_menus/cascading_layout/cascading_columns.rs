use std::mem::take;

use iced::{Element, border::Radius};
use sottomano_config::Config;

use crate::{
    group_items::group_item::{MenuItemAction, MenuParts},
    group_menus::{
        cascading_layout::{
            cascade_row::CascadeDirection, cascade_satellite_panels::CascadeSatellitePanels,
        },
        group_menu_state::{GroupMenuPin, GroupMenuState},
        group_menu_view::{GroupMenuCascadeSurfaces, GroupMenuMessenger, InspectionCard},
        menu_rows::menu_row::{MenuRowContent, MenuRowLook, hovered_menu_row, menu_row_height},
    },
    hover_cards::{PopupMessage, marked_by_hover_target, marked_by_menu_pin},
    widget_groups::{
        widget_group_content::{GroupEvent, GroupItemPin, GroupMenuItemRow},
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        configured_theme::{secondary_text_color, text_color},
        glyph_button,
        icon_glyph::IconGlyph,
        list_end_corners::ListEndCorners,
        phosphor::{Phosphor, Weight},
        uniform_width_column::uniform_width_column,
    },
};

pub(crate) struct GroupMenuGroupRow<'a, M> {
    pub group_identity: WidgetGroupIdentity,
    pub group_title: WidgetGroupTitle,
    pub face: MenuParts<'a, M>,
    pub items: Vec<GroupMenuItemRow<'a, M>>,
}

pub(crate) fn one_row_per_identity<'a, M>(
    rows: impl IntoIterator<Item = GroupMenuGroupRow<'a, M>>,
) -> Vec<GroupMenuGroupRow<'a, M>> {
    rows.into_iter().fold(Vec::new(), |mut merged, row| {
        match merged
            .iter_mut()
            .find(|existing: &&mut GroupMenuGroupRow<'a, M>| {
                existing.group_identity == row.group_identity
            }) {
            Some(existing) => existing.items.extend(row.items),
            None => merged.push(row),
        }
        merged
    })
}

pub(crate) struct GroupMenuRowContext<'p, 'a, M> {
    pub config: &'a Config,
    pub direction: CascadeDirection,
    pub menu: &'p GroupMenuMessenger<'a, M>,
    pub group_menus: &'p GroupMenuState,
    pub cascade_satellite_panels: &'p CascadeSatellitePanels,
}

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
    let open_identity = layout.group_menus.open_group(layout.menu.topic);
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
    let inspected_target = layout
        .group_menus
        .inspected_row(layout.menu.topic)
        .map(|row| &row.target);
    let card_position = inspected_target.and_then(|target| {
        items
            .iter()
            .position(|item| item.action.inspected_target() == target)
    });
    let item_count = items.len();
    let item_corners = layout.cascade_satellite_panels.frame_corners;
    let is_inspected = |item: &GroupMenuItemRow<'a, M>| {
        inspected_target.is_some_and(|target| item.action.inspected_target() == target)
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

fn group_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    entry: GroupMenuGroupRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let icon_size = layout.config.theme.font.icon_size_px;
    let group_title = entry.group_title;
    let pinned_entry = GroupMenuPin::Group(entry.group_identity.clone());
    let pin = marked_by_menu_pin(
        glyph_button(
            Phosphor::PushPin(Weight::Regular),
            icon_size,
            secondary_text_color,
            layout.menu.toggle_pin(pinned_entry.clone()),
        ),
        pinned_entry,
    );
    let caret_color = if look.highlighted {
        text_color
    } else {
        secondary_text_color
    };
    let content = match layout.direction {
        CascadeDirection::Rightward => MenuRowContent {
            leading: Vec::new(),
            parts: entry.face,
            name: group_title.to_string().into(),
            trailing: vec![
                pin,
                IconGlyph::from(Phosphor::CaretRight(Weight::Regular)).view(icon_size, caret_color),
            ],
        },
        CascadeDirection::Leftward => MenuRowContent {
            leading: vec![
                IconGlyph::from(Phosphor::CaretLeft(Weight::Regular)).view(icon_size, caret_color),
            ],
            parts: entry.face,
            name: group_title.to_string().into(),
            trailing: vec![pin],
        },
    };
    let reveal_group = layout.menu.reveal_group(entry.group_identity);
    hovered_menu_row(
        layout.config,
        content,
        look,
        reveal_group.clone(),
        reveal_group,
    )
}

fn item_menu<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    items: Vec<GroupMenuItemRow<'a, M>>,
    is_inspected: &dyn Fn(&GroupMenuItemRow<'a, M>) -> bool,
    panel_corners: Radius,
) -> Element<'a, GroupEvent<M>> {
    let end_corners = ListEndCorners::new(items.len(), panel_corners);
    uniform_width_column(items.into_iter().enumerate().map(|(position, item)| {
        let look = MenuRowLook {
            highlighted: is_inspected(&item),
            corners: end_corners.radius_at(position),
        };
        item_row(layout, group, item, look)
    }))
    .into()
}

fn item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let marked_target = item.action.inspected_target().clone();
    marked_by_hover_target(unmarked_item_row(layout, group, item, look), marked_target)
}

fn unmarked_item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group: &WidgetGroupIdentity,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let item_pin = match item.pin {
        GroupItemPin::Unpinnable => None,
        GroupItemPin::Pinnable(item_key) => Some(glyph_button(
            Phosphor::PushPin(Weight::Regular),
            layout.config.theme.font.icon_size_px,
            secondary_text_color,
            layout.menu.toggle_pin(GroupMenuPin::Item {
                group: group.clone(),
                item_key,
            }),
        )),
    };
    let content = MenuRowContent {
        leading: Vec::new(),
        parts: item.parts,
        name: item.name,
        trailing: item_pin.into_iter().collect(),
    };
    let config = layout.config;
    match item.action {
        MenuItemAction::Inspect(inspected_row) => {
            let inspect = layout.menu.inspect_row(inspected_row);
            hovered_menu_row(config, content, look, inspect.clone(), inspect)
        }
        MenuItemAction::PressAndInspect {
            press,
            inspected_row,
        } => hovered_menu_row(
            config,
            content,
            look,
            GroupEvent::Group(press),
            layout.menu.inspect_row(inspected_row),
        ),
    }
}

#[cfg(test)]
mod tests {
    use iced::widget::Space;

    use super::{GroupMenuGroupRow, WidgetGroupIdentity, one_row_per_identity};
    use crate::{
        group_items::group_item::MenuParts,
        widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupTitle},
    };

    fn row_of(group_identity: WidgetGroupIdentity, title: &str) -> GroupMenuGroupRow<'static, ()> {
        GroupMenuGroupRow {
            group_identity,
            group_title: WidgetGroupTitle::Named(title.to_owned().into()),
            face: MenuParts {
                icon: Space::new().into(),
                value: Space::new().into(),
            },
            items: Vec::new(),
        }
    }

    #[test]
    fn groups_of_one_identity_share_a_row() {
        let gpu = WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu);
        let rows = one_row_per_identity([row_of(gpu.clone(), "GPU"), row_of(gpu, "GPU")]);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn instances_of_one_family_keep_their_own_rows() {
        let rows = one_row_per_identity([
            row_of(
                WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, "sda"),
                "a",
            ),
            row_of(
                WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, "sdb"),
                "b",
            ),
        ]);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn a_player_named_audio_keeps_its_own_row() {
        let rows = one_row_per_identity([
            row_of(WidgetGroupIdentity::sole(WidgetGroupFamily::Audio), "Audio"),
            row_of(
                WidgetGroupIdentity::durable(WidgetGroupFamily::Media, "Audio"),
                "Audio",
            ),
        ]);
        assert_eq!(rows.len(), 2);
    }
}
