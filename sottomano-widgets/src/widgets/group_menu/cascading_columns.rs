use std::mem::take;

use iced::{Element, border::Radius};
use sottomano_config::Config;

use crate::{
    hover_tooltips::PopupMessage,
    widget_groups::group_menus::GroupMenus,
    widgets::{
        bar_widget::Glyph,
        configured_theme::{secondary_text_color, text_color},
        glyph_button,
        group_item::{MenuItemAction, MenuParts},
        group_menu::{
            GroupMenuSurfaces, InspectionCard, MenuMessenger,
            menu_row::{
                MenuRowContent, MenuRowLook, hovered_menu_row, inert_menu_row, menu_row_height,
                pressable_menu_row,
            },
            satellite_panels::SatellitePanels,
        },
        list_end_corners::ListEndCorners,
        phosphor::{Phosphor, Weight},
        uniform_width_column::uniform_width_column,
        widget_group::{CascadeItem, GroupEvent},
    },
};

pub(super) struct CascadeEntry<'a, M> {
    pub group_title: &'static str,
    pub face: MenuParts<'a, M>,
    pub items: Vec<CascadeItem<'a, M>>,
}

pub(super) fn merged_by_title<'a, M>(
    entries: impl IntoIterator<Item = CascadeEntry<'a, M>>,
) -> Vec<CascadeEntry<'a, M>> {
    entries.into_iter().fold(Vec::new(), |mut merged, entry| {
        match merged
            .iter_mut()
            .find(|existing: &&mut CascadeEntry<'a, M>| existing.group_title == entry.group_title)
        {
            Some(existing) => existing.items.extend(entry.items),
            None => merged.push(entry),
        }
        merged
    })
}

pub(super) struct CascadeLayout<'p, 'a, M> {
    pub config: &'a Config,
    pub menu: &'p MenuMessenger<'a, M>,
    pub group_menus: &'p GroupMenus,
    pub satellite_panels: &'p SatellitePanels,
}

struct OpenGroup<'a, M> {
    member: usize,
    items: Vec<CascadeItem<'a, M>>,
}

pub(super) fn cascading_columns<
    'a,
    M: Clone + 'a,
    Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a,
>(
    mut entries: Vec<CascadeEntry<'a, M>>,
    layout: &CascadeLayout<'_, 'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Option<GroupMenuSurfaces<'a, Outer>> {
    if entries.is_empty() {
        return None;
    }
    let open_group = layout
        .group_menus
        .open_member(layout.menu.kind)
        .filter(|member| *member < entries.len())
        .map(|member| OpenGroup {
            member,
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
    Some(GroupMenuSurfaces {
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
    layout: &CascadeLayout<'_, 'a, M>,
    open: OpenGroup<'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Vec<Element<'a, Outer>> {
    let OpenGroup { member, items } = open;
    let inspected_target = layout
        .group_menus
        .inspected_row(layout.menu.kind)
        .map(|row| &row.target);
    let inspected_position = inspected_target.and_then(|target| {
        items
            .iter()
            .position(|item| item.action.inspected_target() == Some(target))
    });
    let item_corners = layout
        .satellite_panels
        .corners_for_rows(layout.config, items.len());
    let is_inspected = |item: &CascadeItem<'a, M>| {
        inspected_target.is_some_and(|target| item.action.inspected_target() == Some(target))
    };
    let item_list = item_menu(layout, items, &is_inspected, item_corners).map(Outer::from);
    let item_panel = layout
        .satellite_panels
        .beside_row(layout.config, item_list, member);
    let card_panel = inspected_position.and_then(|position| {
        card.view(layout.config).map(|card_view| {
            layout.satellite_panels.beside_row(
                layout.config,
                card_view.map(Outer::from),
                member + position,
            )
        })
    });
    [item_panel].into_iter().chain(card_panel).collect()
}

pub(super) fn cascade_offset(config: &Config, rows_above: usize) -> f32 {
    menu_row_height(config) * rows_above as f32
}

fn group_row<'a, M: Clone + 'a>(
    layout: &CascadeLayout<'_, 'a, M>,
    member: usize,
    entry: CascadeEntry<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let icon_size = layout.config.theme.font.icon_size_px;
    let pin = glyph_button(
        Phosphor::PushPin(Weight::Regular),
        icon_size,
        secondary_text_color,
        layout.menu.toggle_pin(entry.group_title),
    );
    let caret_color = if look.highlighted {
        text_color
    } else {
        secondary_text_color
    };
    let caret = Glyph::from(Phosphor::CaretRight(Weight::Regular)).view(icon_size, caret_color);
    let inspect_member = layout.menu.inspect_member(member);
    hovered_menu_row(
        layout.config,
        MenuRowContent {
            parts: entry.face,
            name: entry.group_title,
            trailing: vec![pin, caret],
        },
        look,
        inspect_member.clone(),
        inspect_member,
    )
}

fn item_menu<'a, M: Clone + 'a>(
    layout: &CascadeLayout<'_, 'a, M>,
    items: Vec<CascadeItem<'a, M>>,
    is_inspected: &dyn Fn(&CascadeItem<'a, M>) -> bool,
    panel_corners: Radius,
) -> Element<'a, GroupEvent<M>> {
    let end_corners = ListEndCorners::new(items.len(), panel_corners);
    uniform_width_column(items.into_iter().enumerate().map(|(position, item)| {
        let look = MenuRowLook {
            highlighted: is_inspected(&item),
            corners: end_corners.radius_at(position),
        };
        item_row(layout, item, look)
    }))
    .into()
}

fn item_row<'a, M: Clone + 'a>(
    layout: &CascadeLayout<'_, 'a, M>,
    item: CascadeItem<'a, M>,
    look: MenuRowLook,
) -> Element<'a, GroupEvent<M>> {
    let content = MenuRowContent {
        parts: item.parts,
        name: item.name,
        trailing: Vec::new(),
    };
    let config = layout.config;
    match item.action {
        MenuItemAction::Inert => inert_menu_row(config, content),
        MenuItemAction::Press(press) => {
            pressable_menu_row(config, content, look, GroupEvent::Group(press))
        }
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
