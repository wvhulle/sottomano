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

pub struct ApplicationMenuEntries<'c, M> {
    pub config: &'c Config,
    pub pixmap_handles: &'c PixmapHandles,
    pub on_activate: &'c dyn Fn(i32) -> M,
}

impl<M: Clone + 'static> ApplicationMenuEntries<'_, M> {
    #[must_use]
    pub fn entries(&self, nodes: &[MenuNode]) -> Vec<MenuEntry<'static, i32, M>> {
        nodes.iter().map(|node| self.entry(node)).collect()
    }

    fn entry(&self, node: &MenuNode) -> MenuEntry<'static, i32, M> {
        match node.kind {
            MenuKind::Separator => MenuEntry::Divider,
            MenuKind::Submenu => {
                MenuSubmenu::new(node.id, self.content(node), self.entries(&node.children)).into()
            }
            MenuKind::Standard | MenuKind::Toggle(_) => {
                let row = MenuRow::new(self.content(node));
                let row = if node.enabled {
                    row.on_press((self.on_activate)(node.id))
                } else {
                    row.emphasis(MenuRowEmphasis::Unavailable)
                };
                MenuItem::new(row).into()
            }
        }
    }

    fn content(&self, node: &MenuNode) -> MenuRowContent<'static, M> {
        let icon_size = self.config.theme.font.icon_size_px;
        let content = MenuRowContent::new(node.label.clone().unwrap_or_default());
        match &node.icon {
            Some(source) => {
                content.icon(self.pixmap_handles.icon_handle(source).view(icon_size, 1.0))
            }
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
}
