use std::sync::Arc;

use image::RgbaImage;
use system_tray::menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu};

use crate::{
    desktop_windows_apps::{
        desktop_entry_app_icons::DesktopEntries,
        status_notifier_tray::{IconSource, capped_to_side_limit},
    },
    non_empty_text::non_empty,
};

pub(crate) const SYMBOLIC_ICON_SUFFIX: &str = "-symbolic";

#[derive(Debug, Clone, PartialEq)]
pub struct MenuNode {
    pub id: i32,
    pub label: Option<String>,
    pub enabled: bool,
    pub kind: MenuKind,
    pub icon: Option<IconSource>,
    pub children: Vec<MenuNode>,
}

impl MenuNode {
    pub fn pixmaps(&self) -> Vec<&Arc<RgbaImage>> {
        self.icon
            .iter()
            .filter_map(IconSource::pixmap)
            .chain(self.children.iter().flat_map(MenuNode::pixmaps))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Standard,
    Separator,
    Submenu,
    Toggle(bool),
}

fn menu_item_icon(menu_item: &MenuItem, entries: &DesktopEntries) -> Option<IconSource> {
    if let Some(decoded) = menu_item.icon_data.as_deref().and_then(|icon_bytes| {
        image::load_from_memory(icon_bytes)
            .inspect_err(|decode_failure| {
                tracing::warn!(
                    "menu item icon: cannot decode icon data of {:?}: {decode_failure}",
                    menu_item.label
                );
            })
            .ok()
    }) {
        return Some(IconSource::Pixmap(Arc::new(capped_to_side_limit(
            super::icon_whitespace_trim::trim_rgba(decoded.into_rgba8()),
        ))));
    }
    let icon_name = menu_item
        .icon_name
        .as_deref()
        .filter(|icon_name| !icon_name.is_empty())?;
    symbolic_first_icon_names(icon_name)
        .into_iter()
        .find_map(|candidate_name| entries.theme_icon(&candidate_name))
        .and_then(|icon_path| IconSource::from_theme_file(&icon_path))
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

pub fn visible_menu_node(menu_item: &MenuItem, entries: &DesktopEntries) -> Option<MenuNode> {
    if !menu_item.visible {
        return None;
    }
    let children: Vec<MenuNode> = menu_item
        .submenu
        .iter()
        .filter_map(|child_item| visible_menu_node(child_item, entries))
        .collect();
    let kind = if menu_item.menu_type == MenuType::Separator {
        MenuKind::Separator
    } else if !children.is_empty() || menu_item.children_display.as_deref() == Some("submenu") {
        MenuKind::Submenu
    } else if menu_item.toggle_type != ToggleType::CannotBeToggled {
        MenuKind::Toggle(menu_item.toggle_state == ToggleState::On)
    } else {
        MenuKind::Standard
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
        icon: menu_item_icon(menu_item, entries),
        children,
    })
}

#[must_use]
pub fn tray_menu_nodes(tray_menu: &TrayMenu, entries: &DesktopEntries) -> Vec<MenuNode> {
    tray_menu
        .submenus
        .iter()
        .filter_map(|menu_item| visible_menu_node(menu_item, entries))
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
}
