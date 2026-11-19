use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuRowEmphasis, MenuSubmenu},
    value_controls::{SwitchResponse, TogglePosition, radio_mark, toggle_switch},
};
use sottomano_desktop_sources::desktop_windows_apps::{
    application_menu_nodes::{MenuIconLookup, MenuKind, MenuNode, ToggleCheck, ToggleMark},
    desktop_entry_app_icons::IconTheme,
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

#[must_use]
pub fn menu_icon_lookup(icon_theme: IconTheme, config: &Config) -> MenuIconLookup {
    MenuIconLookup {
        icon_theme,
        size_px: config.theme.font.icon_size_px.round() as u16,
    }
}

const SMALLEST_FRACTION_OF_TARGET: f32 = 0.75;

fn crisp_icon_side_px(design_side_px: f32, target_side_px: f32) -> f32 {
    if design_side_px <= target_side_px {
        return design_side_px * (target_side_px / design_side_px).floor();
    }
    let whole_fraction_px = design_side_px / (design_side_px / target_side_px).ceil();
    if whole_fraction_px >= target_side_px * SMALLEST_FRACTION_OF_TARGET {
        whole_fraction_px
    } else {
        target_side_px
    }
}

fn node_row_content<Message: 'static>(
    node: &MenuNode,
    config: &Config,
    pixmap_handles: &PixmapHandles,
) -> MenuRowContent<'static, Message> {
    let icon_size = config.theme.font.icon_size_px;
    let content = MenuRowContent::new(node.label.clone().unwrap_or_default());
    let content = match &node.icon {
        Some(menu_icon) => content.icon(
            pixmap_handles
                .icon_handle(&menu_icon.source)
                .view(crisp_icon_side_px(menu_icon.design_side_px, icon_size), 1.0),
        ),
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

#[cfg(test)]
mod tests {
    use super::crisp_icon_side_px;

    fn drawn_side(design_side_px: f32, target_side_px: f32) -> f32 {
        crisp_icon_side_px(design_side_px, target_side_px)
    }

    #[test]
    fn small_designs_draw_at_whole_multiples_of_their_size() {
        assert!((drawn_side(16.0, 17.7) - 16.0).abs() < f32::EPSILON);
        assert!((drawn_side(16.0, 33.0) - 32.0).abs() < f32::EPSILON);
        assert!((drawn_side(22.0, 24.0) - 22.0).abs() < f32::EPSILON);
    }

    #[test]
    fn large_designs_draw_at_whole_fractions_of_their_size() {
        assert!((drawn_side(48.0, 17.7) - 16.0).abs() < f32::EPSILON);
        assert!((drawn_side(32.0, 24.0) - 24.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_design_slightly_above_the_target_shrinks_to_the_target() {
        assert!((drawn_side(16.0, 12.0) - 12.0).abs() < f32::EPSILON);
    }
}
