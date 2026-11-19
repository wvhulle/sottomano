use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuRowEmphasis, MenuSubmenu},
    value_controls::{SwitchResponse, TogglePosition, radio_mark, toggle_switch},
};
use sottomano_desktop_sources::desktop_windows_apps::application_menu_nodes::{
    MenuKind, MenuNode, ToggleCheck, ToggleMark,
};
use sottomano_toml_config::Config;

use crate::widgets::icon_handle::PixmapHandles;

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
    pixmap_handles: &PixmapHandles,
    on_activate: &dyn Fn(i32) -> Message,
) -> Vec<MenuEntry<'static, i32, Message>> {
    nodes
        .iter()
        .map(|node| match node.kind {
            MenuKind::Separator => MenuEntry::Divider,
            MenuKind::Submenu => MenuSubmenu::new(
                node.id,
                node_row_content(node, config, pixmap_handles),
                menu_node_entries(&node.children, config, pixmap_handles, on_activate),
            )
            .into(),
            MenuKind::Standard | MenuKind::Toggle { .. } => {
                let row = MenuRow::new(node_row_content(node, config, pixmap_handles));
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
    pixmap_handles: &PixmapHandles,
) -> MenuRowContent<'static, Message> {
    let icon_size = config.theme.font.icon_size_px;
    let content = MenuRowContent::new(node.label.clone().unwrap_or_default());
    let content = match &node.icon {
        Some(source) => content.icon(pixmap_handles.icon_handle(source).view(icon_size, 1.0)),
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
