use std::mem::take;

use iced::{Element, border::Radius};
use sottomano_config::Config;

use crate::{
    hover_tooltips::{PopupMessage, marked_by_hover_target, marked_by_menu_pin},
    widget_groups::group_menus::{GroupMenuPin, GroupMenuState},
    widgets::{
        cascade_row::CascadeDirection,
        configured_theme::{secondary_text_color, text_color},
        glyph_button,
        group_item::{MenuItemAction, MenuParts},
        group_menu::{
            GroupMenuCascadeSurfaces, GroupMenuMessenger, InspectionCard,
            menu_row::{MenuRowContent, MenuRowLook, hovered_menu_row, menu_row_height},
            satellite_panels::CascadeSatellitePanels,
        },
        icon_glyph::IconGlyph,
        list_end_corners::ListEndCorners,
        phosphor::{Phosphor, Weight},
        uniform_width_column::uniform_width_column,
        widget_group::{
            GroupEvent, GroupItemPin, GroupMenuItemRow, GroupTitleClaim, TitleRow, WidgetGroupTitle,
        },
    },
};

pub(super) struct GroupMenuGroupRow<'a, M> {
    pub title_claim: GroupTitleClaim,
    pub face: MenuParts<'a, M>,
    pub items: Vec<GroupMenuItemRow<'a, M>>,
}

pub(super) fn merged_where_requested<'a, M>(
    entries: impl IntoIterator<Item = GroupMenuGroupRow<'a, M>>,
) -> Vec<GroupMenuGroupRow<'a, M>> {
    entries.into_iter().fold(Vec::new(), |mut merged, entry| {
        match merged
            .iter_mut()
            .find(|existing: &&mut GroupMenuGroupRow<'a, M>| {
                entry.title_claim.title_row == TitleRow::SharedWithSameTitle
                    && existing.title_claim == entry.title_claim
            }) {
            Some(existing) => existing.items.extend(entry.items),
            None => merged.push(entry),
        }
        merged
    })
}

pub(super) fn duplicate_group_titles(
    title_claims: impl IntoIterator<Item = GroupTitleClaim>,
) -> Vec<WidgetGroupTitle> {
    let title_claims: Vec<GroupTitleClaim> = title_claims.into_iter().collect();
    title_claims
        .iter()
        .enumerate()
        .filter(|(position, claim)| {
            title_claims[..*position].iter().any(|earlier_claim| {
                earlier_claim.group_title == claim.group_title
                    && (earlier_claim.title_row == TitleRow::OwnRow
                        || claim.title_row == TitleRow::OwnRow)
            })
        })
        .map(|(_, claim)| claim.group_title.clone())
        .collect()
}

pub(super) struct GroupMenuRowContext<'p, 'a, M> {
    pub config: &'a Config,
    pub direction: CascadeDirection,
    pub menu: &'p GroupMenuMessenger<'a, M>,
    pub group_menus: &'p GroupMenuState,
    pub satellite_panels: &'p CascadeSatellitePanels,
}

struct OpenGroup<'a, M> {
    member: usize,
    group_title: WidgetGroupTitle,
    items: Vec<GroupMenuItemRow<'a, M>>,
}

pub(super) fn cascading_columns<
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
    let open_group = layout
        .group_menus
        .open_member(layout.menu.topic)
        .filter(|member| *member < entries.len())
        .map(|member| OpenGroup {
            member,
            group_title: entries[member].title_claim.group_title.clone(),
            items: take(&mut entries[member].items),
        });
    let open_member = open_group.as_ref().map(|open| open.member);
    let group_corners = ListEndCorners::new(entries.len(), layout.satellite_panels.frame_corners);
    let group_rows = entries.into_iter().enumerate().map(|(member, entry)| {
        let look = MenuRowLook {
            highlighted: open_member == Some(member),
            corners: group_corners.radius_at(member),
        };
        group_row(layout, member, entry, look)
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
        member,
        group_title,
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
    let item_corners = layout.satellite_panels.frame_corners;
    let is_inspected = |item: &GroupMenuItemRow<'a, M>| {
        inspected_target.is_some_and(|target| item.action.inspected_target() == target)
    };
    let item_list =
        item_menu(layout, &group_title, items, &is_inspected, item_corners).map(Outer::from);
    let item_satellite = layout.satellite_panels.beside_row(
        layout.config,
        item_list,
        cascade_offset(layout.config, member),
    );
    let card_satellite = card_position.and_then(|position| {
        card.view(layout.config).map(|card_view| {
            let card_row_top =
                layout
                    .satellite_panels
                    .item_row_top(layout.config, member, position, item_count);
            layout.satellite_panels.beside_row(
                layout.config,
                card_view.map(Outer::from),
                card_row_top,
            )
        })
    });
    [item_satellite].into_iter().chain(card_satellite).collect()
}

pub(super) fn cascade_offset(config: &Config, rows_above: usize) -> f32 {
    menu_row_height(config) * rows_above as f32
}

fn group_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    member: usize,
    entry: GroupMenuGroupRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let icon_size = layout.config.theme.font.icon_size_px;
    let group_title = entry.title_claim.group_title;
    let pinned_entry = GroupMenuPin::Group {
        group_title: group_title.clone(),
    };
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
    let inspect_member = layout.menu.inspect_member(member);
    hovered_menu_row(
        layout.config,
        content,
        look,
        inspect_member.clone(),
        inspect_member,
    )
}

fn item_menu<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group_title: &WidgetGroupTitle,
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
        item_row(layout, group_title, item, look)
    }))
    .into()
}

fn item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group_title: &WidgetGroupTitle,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let marked_target = item.action.inspected_target().clone();
    marked_by_hover_target(
        unmarked_item_row(layout, group_title, item, look),
        marked_target,
    )
}

fn unmarked_item_row<'a, M: Clone + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    group_title: &WidgetGroupTitle,
    item: GroupMenuItemRow<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let item_pin = match item.pin {
        GroupItemPin::Unpinnable => None,
        GroupItemPin::Pinnable(item_name) => Some(glyph_button(
            Phosphor::PushPin(Weight::Regular),
            layout.config.theme.font.icon_size_px,
            secondary_text_color,
            layout.menu.toggle_pin(GroupMenuPin::Item {
                group_title: group_title.clone(),
                item_name,
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

    use super::{
        GroupMenuGroupRow, GroupTitleClaim, TitleRow, WidgetGroupTitle, duplicate_group_titles,
        merged_where_requested,
    };
    use crate::widgets::group_item::MenuParts;

    const ETHERNET_TITLE: WidgetGroupTitle = WidgetGroupTitle::fixed("Ethernet");
    const DRIVE_TITLE: WidgetGroupTitle = WidgetGroupTitle::fixed("Drive");

    fn own_row(group_title: WidgetGroupTitle) -> GroupTitleClaim {
        GroupTitleClaim {
            group_title,
            title_row: TitleRow::OwnRow,
        }
    }

    fn shared_row(group_title: WidgetGroupTitle) -> GroupTitleClaim {
        GroupTitleClaim {
            group_title,
            title_row: TitleRow::SharedWithSameTitle,
        }
    }

    fn entry_claiming(title_claim: GroupTitleClaim) -> GroupMenuGroupRow<'static, ()> {
        GroupMenuGroupRow {
            title_claim,
            face: MenuParts {
                icon: Space::new().into(),
                value: Space::new().into(),
            },
            items: Vec::new(),
        }
    }

    fn merged_row_count(title_claims: [GroupTitleClaim; 2]) -> usize {
        merged_where_requested(title_claims.map(entry_claiming)).len()
    }

    #[test]
    fn an_own_row_duplicate_is_flagged() {
        assert_eq!(
            duplicate_group_titles([own_row(ETHERNET_TITLE), own_row(ETHERNET_TITLE)]),
            [ETHERNET_TITLE]
        );
    }

    #[test]
    fn a_duplicate_with_only_one_side_sharing_is_flagged() {
        assert_eq!(
            duplicate_group_titles([shared_row(ETHERNET_TITLE), own_row(ETHERNET_TITLE)]),
            [ETHERNET_TITLE]
        );
    }

    #[test]
    fn requested_sharing_and_distinct_titles_are_not_flagged() {
        assert!(
            duplicate_group_titles([shared_row(DRIVE_TITLE), shared_row(DRIVE_TITLE)]).is_empty()
        );
        assert!(duplicate_group_titles([own_row(DRIVE_TITLE), own_row(ETHERNET_TITLE)]).is_empty());
    }

    #[test]
    fn only_groups_requesting_it_share_a_row() {
        assert_eq!(
            merged_row_count([shared_row(DRIVE_TITLE), shared_row(DRIVE_TITLE)]),
            1
        );
        assert_eq!(
            merged_row_count([own_row(ETHERNET_TITLE), own_row(ETHERNET_TITLE)]),
            2
        );
        assert_eq!(
            merged_row_count([shared_row(DRIVE_TITLE), own_row(DRIVE_TITLE)]),
            2
        );
    }
}
