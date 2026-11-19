use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent, MenuRowEmphasis, MenuSubmenu},
    theming::text_color,
};
use sottomano_desktop_sources::desktop_windows_apps::application_menu_nodes::{MenuKind, MenuNode};
use sottomano_toml_config::Config;

use crate::widgets::{
    icon_glyph::IconGlyph,
    icon_handle::PixmapHandles,
    phosphor::{Phosphor, Weight},
};

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
            MenuKind::Standard | MenuKind::Toggle(_) => {
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
    match &node.icon {
        Some(source) => content.icon(pixmap_handles.icon_handle(source).view(icon_size, 1.0)),
        None => match node.kind {
            MenuKind::Toggle(true) => content.icon(
                IconGlyph::from(Phosphor::Check(Weight::Regular)).view(icon_size, text_color),
            ),
            MenuKind::Toggle(false)
            | MenuKind::Standard
            | MenuKind::Submenu
            | MenuKind::Separator => content,
        },
    }
}
