use std::{collections::HashSet, iter::once};

use sottomano_desktop_sources::desktop_windows_apps::application_menu_nodes::{MenuKind, MenuNode};
use sottomano_toml_config::Config;

use crate::widgets::{
    configured_theme::text_color,
    context_menu::{ContextMenuLine, ContextMenuRow, ContextMenuRowPress, ContextMenuTrailing},
    icon_glyph::IconGlyph,
    icon_handle::PixmapHandles,
    phosphor::{Phosphor, Weight},
};

pub struct ApplicationMenuLines<'c, M> {
    pub config: &'c Config,
    pub pixmap_handles: &'c PixmapHandles,
    pub open: &'c HashSet<i32>,
    pub on_activate: &'c dyn Fn(i32) -> M,
    pub on_toggle: &'c dyn Fn(i32) -> M,
}

impl<'c, M: Clone + 'c> ApplicationMenuLines<'c, M> {
    #[must_use]
    pub fn lines<'a>(&self, nodes: &[MenuNode]) -> Vec<ContextMenuLine<'a, M>>
    where
        M: 'a,
    {
        self.nested(0, nodes)
    }

    fn nested<'a>(&self, depth: usize, nodes: &[MenuNode]) -> Vec<ContextMenuLine<'a, M>>
    where
        M: 'a,
    {
        nodes
            .iter()
            .flat_map(|node| self.node_lines(depth, node))
            .collect()
    }

    fn node_lines<'a>(&self, depth: usize, node: &MenuNode) -> Vec<ContextMenuLine<'a, M>>
    where
        M: 'a,
    {
        match node.kind {
            MenuKind::Separator => vec![ContextMenuLine::Divider],
            MenuKind::Submenu => {
                let open = self.open.contains(&node.id);
                let head = self.row(
                    depth,
                    node,
                    ContextMenuTrailing::Submenu { open },
                    (self.on_toggle)(node.id),
                );
                if open {
                    once(head)
                        .chain(self.nested(depth + 1, &node.children))
                        .collect()
                } else {
                    vec![head]
                }
            }
            MenuKind::Standard | MenuKind::Toggle(_) => {
                vec![self.row(
                    depth,
                    node,
                    ContextMenuTrailing::Nothing,
                    (self.on_activate)(node.id),
                )]
            }
        }
    }

    fn row<'a>(
        &self,
        depth: usize,
        node: &MenuNode,
        trailing: ContextMenuTrailing,
        on_press: M,
    ) -> ContextMenuLine<'a, M>
    where
        M: 'a,
    {
        let icon_size = self.config.theme.font.icon_size_px;
        let leading = node
            .icon
            .as_ref()
            .map(|source| self.pixmap_handles.icon_handle(source).view(icon_size, 1.0))
            .or_else(|| {
                matches!(node.kind, MenuKind::Toggle(true)).then(|| {
                    IconGlyph::from(Phosphor::Check(Weight::Regular)).view(icon_size, text_color)
                })
            });
        ContextMenuLine::Row(ContextMenuRow {
            leading,
            label: node.label.clone(),
            trailing,
            depth,
            press: if node.enabled {
                ContextMenuRowPress::Enabled(on_press)
            } else {
                ContextMenuRowPress::Disabled
            },
        })
    }
}
