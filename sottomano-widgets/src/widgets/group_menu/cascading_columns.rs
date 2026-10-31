use iced::{Element, border::Radius};
use sottomano_config::Config;

use crate::{
    hover_tooltips::{HoverTarget, TooltipMessage},
    widgets::{
        bar_widget::Glyph,
        configured_theme::{secondary_text_color, text_color},
        glyph_button,
        group_item::MenuParts,
        group_menu::{
            GroupMenuSurfaces, MenuInspection, PinToggle,
            menu_row::{
                MenuRowAction, MenuRowContent, inert_menu_row, inspect_row_message, menu_row,
                menu_row_height,
            },
            satellite_panels::SatellitePanels,
        },
        list_end_corners::ListEndCorners,
        phosphor::{Phosphor, Weight},
        uniform_width_column::uniform_width_column,
        widget_group::{CascadeItem, GroupEvent},
    },
};

pub(super) struct CascadeDevice<'a, M> {
    pub group_title: &'static str,
    pub face: MenuParts<'a, M>,
    pub items: Vec<CascadeItem<'a, M>>,
}

pub(super) fn merged_by_title<'a, M>(
    devices: Vec<CascadeDevice<'a, M>>,
) -> Vec<CascadeDevice<'a, M>> {
    devices.into_iter().fold(Vec::new(), |mut entries, device| {
        match entries
            .iter_mut()
            .find(|entry: &&mut CascadeDevice<'a, M>| entry.group_title == device.group_title)
        {
            Some(entry) => entry.items.extend(device.items),
            None => entries.push(device),
        }
        entries
    })
}

pub(super) fn cascading_columns<'a, M: Clone + 'a, Outer: From<GroupEvent<M>> + 'a>(
    entries: Vec<CascadeDevice<'a, M>>,
    on_pin_toggle: &PinToggle<'a, M>,
    config: &'a Config,
    satellite_panels: &SatellitePanels,
    inspection: &MenuInspection<'_>,
    card: Option<Element<'a, Outer>>,
) -> Option<GroupMenuSurfaces<'a, Outer>> {
    if entries.is_empty() {
        return None;
    }
    let open_member = inspection
        .open_member
        .filter(|member| *member < entries.len());
    let group_corners = ListEndCorners::new(entries.len(), satellite_panels.frame_corners);
    let (group_rows, mut item_lists): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .enumerate()
        .map(|(member, entry)| {
            let CascadeDevice {
                group_title,
                face,
                items,
            } = entry;
            let row = group_row(
                config,
                member,
                GroupRowFace { group_title, face },
                GroupRowState {
                    open: open_member == Some(member),
                    corners: group_corners.radius_at(member),
                },
                on_pin_toggle(group_title),
            );
            (row, items)
        })
        .unzip();
    let mut satellites: Vec<Element<'a, Outer>> = Vec::new();
    if let Some(member) = open_member {
        let items = item_lists.swap_remove(member);
        let item_count = items.len();
        let inspected_target = inspection.inspected_row.map(|row| &row.target);
        let inspected_position = inspected_target.and_then(|target| {
            items.iter().position(|item| {
                item.inspected_row
                    .as_ref()
                    .is_some_and(|row| &row.target == target)
            })
        });
        let item_menu = item_menu(
            config,
            items,
            inspected_target,
            satellite_panels.corners_for_rows(config, item_count),
        );
        satellites.push(satellite_panels.beside_row(config, item_menu.map(Outer::from), member));
        if let (Some(card), Some(item_position)) = (card, inspected_position) {
            satellites.push(satellite_panels.beside_row(config, card, member + item_position));
        }
    }
    Some(GroupMenuSurfaces {
        primary: Element::from(uniform_width_column(group_rows)).map(Outer::from),
        satellites,
    })
}

pub(super) fn cascade_offset(config: &Config, rows_above: usize) -> f32 {
    menu_row_height(config) * rows_above as f32
}

#[derive(Clone, Copy)]
pub(super) struct GroupRowState {
    open: bool,
    corners: Radius,
}

pub(super) struct GroupRowFace<'a, M> {
    pub group_title: &'static str,
    pub face: MenuParts<'a, M>,
}

pub(super) fn group_row<'a, M: Clone + 'a>(
    config: &'a Config,
    member: usize,
    row_face: GroupRowFace<'a, M>,
    row_state: GroupRowState,
    pin_toggle: M,
) -> Element<'a, GroupEvent<M>> {
    let GroupRowState { open, corners } = row_state;
    let icon_size = config.theme.font.icon_size_px;
    let inspect_member = GroupEvent::Tooltip(TooltipMessage::InspectMember(member));
    let pin = glyph_button(
        Phosphor::PushPin(Weight::Regular),
        icon_size,
        secondary_text_color,
        GroupEvent::Group(pin_toggle),
    );
    let caret_color = if open {
        text_color
    } else {
        secondary_text_color
    };
    let caret = Glyph::from(Phosphor::CaretRight(Weight::Regular)).view(icon_size, caret_color);
    menu_row(
        config,
        MenuRowContent {
            parts: row_face.face,
            name: row_face.group_title,
            trailing: vec![pin, caret],
        },
        MenuRowAction {
            message: inspect_member.clone(),
            on_enter: Some(inspect_member),
            highlighted: open,
            corners,
        },
    )
}

pub(super) fn item_menu<'a, M: Clone + 'a>(
    config: &'a Config,
    items: Vec<CascadeItem<'a, M>>,
    inspected_target: Option<&HoverTarget>,
    frame_corners: Radius,
) -> Element<'a, GroupEvent<M>> {
    let end_corners = ListEndCorners::new(items.len(), frame_corners);
    uniform_width_column(items.into_iter().enumerate().map(|(position, item)| {
        item_row(
            config,
            item,
            inspected_target,
            end_corners.radius_at(position),
        )
    }))
    .into()
}

fn item_row<'a, M: Clone + 'a>(
    config: &'a Config,
    item: CascadeItem<'a, M>,
    inspected_target: Option<&HoverTarget>,
    corners: Radius,
) -> Element<'a, GroupEvent<M>> {
    let highlighted = item
        .inspected_row
        .as_ref()
        .is_some_and(|row| Some(&row.target) == inspected_target);
    let content = MenuRowContent {
        parts: item.parts,
        name: item.name,
        trailing: Vec::new(),
    };
    let action = match (item.press, item.inspected_row) {
        (Some(press), inspected_row) => MenuRowAction {
            message: GroupEvent::Group(press),
            on_enter: inspected_row.map(inspect_row_message),
            highlighted,
            corners,
        },
        (None, Some(inspected_row)) => MenuRowAction {
            message: inspect_row_message(inspected_row.clone()),
            on_enter: Some(inspect_row_message(inspected_row)),
            highlighted,
            corners,
        },
        (None, None) => return inert_menu_row(config, content),
    };
    menu_row(config, content, action)
}
