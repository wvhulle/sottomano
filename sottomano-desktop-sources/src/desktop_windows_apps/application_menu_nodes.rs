use std::sync::Arc;

use image::RgbaImage;
use system_tray::menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu};

use crate::{
    desktop_windows_apps::{
        desktop_entry_app_icons::{DesktopEntries, IconTheme},
        icon_whitespace_trim::IconTrim,
        status_notifier_tray::capped_to_side_limit,
        theme_icon_sources::IconSource,
    },
    non_empty_text::non_empty,
};

pub(crate) const SYMBOLIC_ICON_SUFFIX: &str = "-symbolic";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MenuIconLookup {
    pub icon_theme: IconTheme,
    pub size_px: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuNodeIcon {
    pub source: IconSource,
    pub design_side_px: f32,
}

impl MenuNodeIcon {
    fn measured(source: IconSource) -> Option<Self> {
        source
            .design_side_px()
            .inspect_err(|measure_failure| {
                tracing::warn!("menu item icon: cannot measure its design size: {measure_failure}");
            })
            .ok()
            .map(|design_side_px| Self {
                source,
                design_side_px,
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuNode {
    pub id: i32,
    pub label: Option<String>,
    pub enabled: bool,
    pub kind: MenuKind,
    pub icon: Option<MenuNodeIcon>,
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    pub fn pixmaps(&self) -> Vec<&Arc<RgbaImage>> {
        self.icon
            .iter()
            .filter_map(|menu_icon| menu_icon.source.pixmap())
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

fn named_menu_icon(
    menu_item: &MenuItem,
    entries: &DesktopEntries,
    menu_icon_px: u16,
) -> Option<IconSource> {
    let icon_name = menu_item
        .icon_name
        .as_deref()
        .filter(|icon_name| !icon_name.is_empty())?;
    symbolic_first_icon_names(icon_name)
        .into_iter()
        .find_map(|candidate_name| entries.theme_lookup_at(&candidate_name, menu_icon_px))
        .and_then(|icon_path| IconSource::logged_theme_icon(&icon_path, IconTrim::DesignSize))
}

fn embedded_menu_icon(menu_item: &MenuItem) -> Option<IconSource> {
    let icon_bytes = menu_item.icon_data.as_deref()?;
    image::load_from_memory(icon_bytes)
        .inspect_err(|decode_failure| {
            tracing::warn!(
                "menu item icon: cannot decode icon data of {:?}: {decode_failure}",
                menu_item.label
            );
        })
        .ok()
        .map(|decoded| IconSource::Pixmap(Arc::new(capped_to_side_limit(decoded.into_rgba8()))))
}

pub(crate) fn symbolic_first_icon_names(icon_name: &str) -> Vec<String> {
    if icon_name.ends_with(SYMBOLIC_ICON_SUFFIX) {
        vec![icon_name.to_string()]
    } else {
        vec![
            format!("{icon_name}{SYMBOLIC_ICON_SUFFIX}"),
            icon_name.to_string(),
        ]
    }
}

pub fn visible_menu_node(
    menu_item: &MenuItem,
    entries: &DesktopEntries,
    menu_icon_px: u16,
) -> Option<MenuNode> {
    if !menu_item.visible {
        return None;
    }
    let children: Vec<MenuNode> = menu_item
        .submenu
        .iter()
        .filter_map(|child_item| visible_menu_node(child_item, entries, menu_icon_px))
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
            .and_then(non_empty),
        enabled: menu_item.enabled,
        kind,
        icon: named_menu_icon(menu_item, entries, menu_icon_px)
            .or_else(|| embedded_menu_icon(menu_item))
            .and_then(MenuNodeIcon::measured),
        children,
    })
}

#[must_use]
pub fn tray_menu_nodes(
    tray_menu: &TrayMenu,
    entries: &DesktopEntries,
    menu_icon_px: u16,
) -> Vec<MenuNode> {
    tray_menu
        .submenus
        .iter()
        .filter_map(|menu_item| visible_menu_node(menu_item, entries, menu_icon_px))
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
    fn plain_icon_name_tries_its_symbolic_variant_first() {
        assert_eq!(
            symbolic_first_icon_names("nm-device-wired-secure"),
            ["nm-device-wired-secure-symbolic", "nm-device-wired-secure"]
        );
    }

    #[test]
    fn symbolic_icon_name_is_looked_up_once() {
        assert_eq!(
            symbolic_first_icon_names("network-wired-symbolic"),
            ["network-wired-symbolic"]
        );
    }

    #[test]
    fn mnemonic_underscores_are_dropped_and_doubled_ones_kept() {
        assert_eq!(label_without_mnemonics("_File"), "File");
        assert_eq!(label_without_mnemonics("snake__case"), "snake_case");
    }

    const TEST_MENU_ICON_PX: u16 = 18;

    fn toggle_item_kind(toggle_type: ToggleType, toggle_state: ToggleState) -> MenuKind {
        let toggle_item = MenuItem {
            label: Some("Trim Margins".to_owned()),
            visible: true,
            enabled: true,
            toggle_type,
            toggle_state,
            ..MenuItem::default()
        };
        visible_menu_node(&toggle_item, &DesktopEntries::default(), TEST_MENU_ICON_PX)
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
