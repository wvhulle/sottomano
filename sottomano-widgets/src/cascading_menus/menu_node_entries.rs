use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuRowEmphasis, MenuSubmenu},
    value_controls::{SwitchResponse, TogglePosition, radio_mark, toggle_switch},
};
use iced::Element;
use sottomano_desktop_sources::{
    desktop_windows_apps::application_menu_nodes::{
        MenuItemIcon, MenuKind, MenuNode, ToggleCheck, ToggleMark,
    },
    theme_icons::icon_requests::{IconRequest, menu_row_icon_request},
};
use sottomano_toml_config::Config;

use crate::icon_catalog::{
    crisp_icon_size::crisp_icon_side_px, drawable_icon_catalog::DrawableIconSources,
};

fn menu_row_icon<Message: 'static>(
    icon_sources: DrawableIconSources<'_>,
    menu_item_icon: &MenuItemIcon,
    config: &Config,
) -> Option<Element<'static, Message>> {
    let target_side_px = config.theme.font.icon_size_px;
    match menu_item_icon {
        MenuItemIcon::NoIcon => None,
        MenuItemIcon::Named(icon_name) => {
            let drawable_icon = icon_sources
                .icon_catalog
                .drawable_icon(&menu_row_icon_request(icon_name, menu_icon_side_px(config)))?;
            Some(drawable_icon.view(
                crisp_icon_side_px(drawable_icon.design_side_px, target_side_px),
                1.0,
            ))
        }
        MenuItemIcon::Embedded(pixels) => {
            let design_side_px = pixels.width().max(pixels.height()) as f32;
            Some(
                icon_sources
                    .pixmap_handles
                    .pixmap_handle(pixels)
                    .view(crisp_icon_side_px(design_side_px, target_side_px), 1.0),
            )
        }
    }
}

#[must_use]
pub fn menu_icon_side_px(config: &Config) -> u16 {
    config.theme.font.icon_size_px.round() as u16
}

#[must_use]
pub fn menu_node_icon_requests(nodes: &[MenuNode], menu_icon_side_px: u16) -> Vec<IconRequest> {
    nodes
        .iter()
        .flat_map(|node| {
            let own_request = match &node.icon {
                MenuItemIcon::Named(icon_name) => {
                    Some(menu_row_icon_request(icon_name, menu_icon_side_px))
                }
                MenuItemIcon::NoIcon | MenuItemIcon::Embedded(_) => None,
            };
            own_request
                .into_iter()
                .chain(menu_node_icon_requests(&node.children, menu_icon_side_px))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressDismissal {
    ClosesMenu,
    KeepsMenuOpen,
}

#[must_use]
pub fn press_dismissal(nodes: &[MenuNode], item_id: i32) -> PressDismissal {
    match pressed_node(nodes, item_id).map(|node| node.kind) {
        Some(MenuKind::Toggle { .. }) => PressDismissal::KeepsMenuOpen,
        Some(MenuKind::Standard | MenuKind::Submenu | MenuKind::Separator) | None => {
            PressDismissal::ClosesMenu
        }
    }
}

fn pressed_node(nodes: &[MenuNode], item_id: i32) -> Option<&MenuNode> {
    nodes.iter().find_map(|node| {
        if node.id == item_id {
            Some(node)
        } else {
            pressed_node(&node.children, item_id)
        }
    })
}

pub fn menu_node_entries<Message: Clone + 'static>(
    nodes: &[MenuNode],
    config: &Config,
    icon_sources: DrawableIconSources<'_>,
    on_activate: &dyn Fn(i32) -> Message,
) -> Vec<MenuEntry<'static, i32, Message>> {
    nodes
        .iter()
        .map(|node| match node.kind {
            MenuKind::Separator => MenuEntry::Divider,
            MenuKind::Submenu => MenuSubmenu::new(
                node.id,
                node_row_content(node, config, icon_sources),
                menu_node_entries(&node.children, config, icon_sources, on_activate),
            )
            .into(),
            MenuKind::Standard | MenuKind::Toggle { .. } => {
                let row = MenuRow::new(node_row_content(node, config, icon_sources));
                let row = if node.enabled {
                    row.on_press(on_activate(node.id))
                } else {
                    row.emphasis(MenuRowEmphasis::Unavailable)
                };
                MenuItem::new(row).into()
            }
        })
        .collect()
}

fn node_row_content<Message: 'static>(
    node: &MenuNode,
    config: &Config,
    icon_sources: DrawableIconSources<'_>,
) -> MenuRowContent<'static, Message> {
    let icon_size = config.theme.font.icon_size_px;
    let content = MenuRowContent::new(node.label.clone().unwrap_or_default());
    let content = match menu_row_icon(icon_sources, &node.icon, config) {
        Some(row_icon) => content.icon(row_icon),
        None => content,
    };
    let MenuKind::Toggle { mark, check } = node.kind else {
        return content;
    };
    let toggle_position = match check {
        ToggleCheck::Checked => TogglePosition::On,
        ToggleCheck::Unchecked => TogglePosition::Off,
    };
    let response = if node.enabled {
        SwitchResponse::Pressable
    } else {
        SwitchResponse::Inert
    };
    match mark {
        ToggleMark::Checkbox => content.value(
            toggle_switch(toggle_position)
                .size(icon_size)
                .response(response),
        ),
        ToggleMark::Radio => content.value(
            radio_mark(toggle_position)
                .size(icon_size)
                .response(response),
        ),
    }
}
