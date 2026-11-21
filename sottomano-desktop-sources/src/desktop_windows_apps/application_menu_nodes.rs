use std::sync::Arc;

use image::RgbaImage;
use system_tray::menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu};

use crate::{
    desktop_windows_apps::status_notifier_tray::capped_to_side_limit, non_empty_text::non_empty,
};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuItemIcon {
    NoIcon,
    Named(String),
    Embedded(Arc<RgbaImage>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuLabel {
    Labelled(String),
    Unlabelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemSensitivity {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuNode {
    pub id: i32,
    pub label: MenuLabel,
    pub sensitivity: MenuItemSensitivity,
    pub kind: MenuKind,
    pub icon: MenuItemIcon,
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    pub fn pixmaps(&self) -> Vec<&Arc<RgbaImage>> {
        let own_pixmap = match &self.icon {
            MenuItemIcon::Embedded(pixels) => Some(pixels),
            MenuItemIcon::NoIcon | MenuItemIcon::Named(_) => None,
        };
        own_pixmap
            .into_iter()
            .chain(self.children.iter().flat_map(MenuNode::pixmaps))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleMark {
    Checkbox,
    Radio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleCheck {
    Checked,
    Unchecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Standard,
    Separator,
    Submenu,
    Toggle {
        mark: ToggleMark,
        check: ToggleCheck,
    },
}

fn menu_item_icon(menu_item: &MenuItem) -> MenuItemIcon {
    if let Some(icon_name) = menu_item
        .icon_name
        .as_deref()
        .filter(|icon_name| !icon_name.is_empty())
    {
        return MenuItemIcon::Named(icon_name.to_owned());
    }
    let Some(icon_bytes) = menu_item.icon_data.as_deref() else {
        return MenuItemIcon::NoIcon;
    };
    match image::load_from_memory(icon_bytes) {
        Ok(decoded) => MenuItemIcon::Embedded(Arc::new(capped_to_side_limit(decoded.into_rgba8()))),
        Err(decode_failure) => {
            tracing::warn!(
                label = ?menu_item.label,
                %decode_failure,
                "menu item icon: cannot decode its icon data"
            );
            MenuItemIcon::NoIcon
        }
    }
}

pub fn visible_menu_node(menu_item: &MenuItem) -> Option<MenuNode> {
    if !menu_item.visible {
        return None;
    }
    let children: Vec<MenuNode> = menu_item
        .submenu
        .iter()
        .filter_map(visible_menu_node)
        .collect();
    let kind = if menu_item.menu_type == MenuType::Separator {
        MenuKind::Separator
    } else if !children.is_empty() || menu_item.children_display.as_deref() == Some("submenu") {
        MenuKind::Submenu
    } else {
        let check = match menu_item.toggle_state {
            ToggleState::On => ToggleCheck::Checked,
            ToggleState::Off | ToggleState::Indeterminate => ToggleCheck::Unchecked,
        };
        match menu_item.toggle_type {
            ToggleType::Checkmark => MenuKind::Toggle {
                mark: ToggleMark::Checkbox,
                check,
            },
            ToggleType::Radio => MenuKind::Toggle {
                mark: ToggleMark::Radio,
                check,
            },
            ToggleType::CannotBeToggled => MenuKind::Standard,
        }
    };
    Some(MenuNode {
        id: menu_item.id,
        label: menu_item
            .label
            .as_deref()
            .map(label_without_mnemonics)
            .and_then(non_empty)
            .map_or(MenuLabel::Unlabelled, MenuLabel::Labelled),
        sensitivity: if menu_item.enabled {
            MenuItemSensitivity::Enabled
        } else {
            MenuItemSensitivity::Disabled
        },
        kind,
        icon: menu_item_icon(menu_item),
        children,
    })
}

#[must_use]
pub fn tray_menu_nodes(tray_menu: &TrayMenu) -> Vec<MenuNode> {
    tray_menu
        .submenus
        .iter()
        .filter_map(visible_menu_node)
        .collect()
}

#[must_use]
pub fn label_without_mnemonics(raw_label: &str) -> String {
    let mut cleaned_label = String::with_capacity(raw_label.len());
    let mut label_chars = raw_label.chars().peekable();
    while let Some(label_char) = label_chars.next() {
        if label_char == '_' {
            if label_chars.peek() == Some(&'_') {
                cleaned_label.push('_');
                label_chars.next();
            }
        } else {
            cleaned_label.push(label_char);
        }
    }
    cleaned_label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonic_underscores_are_dropped_and_doubled_ones_kept() {
        assert_eq!(label_without_mnemonics("_File"), "File");
        assert_eq!(label_without_mnemonics("snake__case"), "snake_case");
    }

    fn toggle_item_kind(toggle_type: ToggleType, toggle_state: ToggleState) -> MenuKind {
        let toggle_item = MenuItem {
            label: Some("Trim Margins".to_owned()),
            visible: true,
            enabled: true,
            toggle_type,
            toggle_state,
            ..MenuItem::default()
        };
        visible_menu_node(&toggle_item)
            .expect("a visible item becomes a node")
            .kind
    }

    #[test]
    fn checkable_items_keep_their_mark_and_state() {
        assert_eq!(
            toggle_item_kind(ToggleType::Checkmark, ToggleState::Off),
            MenuKind::Toggle {
                mark: ToggleMark::Checkbox,
                check: ToggleCheck::Unchecked
            }
        );
        assert_eq!(
            toggle_item_kind(ToggleType::Radio, ToggleState::On),
            MenuKind::Toggle {
                mark: ToggleMark::Radio,
                check: ToggleCheck::Checked
            }
        );
        assert_eq!(
            toggle_item_kind(ToggleType::CannotBeToggled, ToggleState::On),
            MenuKind::Standard
        );
    }
}
