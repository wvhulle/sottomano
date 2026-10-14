use std::{collections::HashSet, iter::once};

use crate::{
    config::Config,
    data::source::tray::{MenuKind, MenuNode},
    widgets::{
        bar_widget::Glyph,
        context_menu::{MenuLine, MenuRow, Trailing},
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
    },
};

pub struct TrayMenuLines<'c, M> {
    pub config: &'c Config,
    pub open: &'c HashSet<i32>,
    pub on_activate: &'c dyn Fn(i32) -> M,
    pub on_toggle: &'c dyn Fn(i32) -> M,
}

impl<'c, M: Clone + 'c> TrayMenuLines<'c, M> {
    pub fn lines<'a>(&self, nodes: &[MenuNode]) -> Vec<MenuLine<'a, M>>
    where
        M: 'a,
    {
        self.nested(0, nodes)
    }

    fn nested<'a>(&self, depth: usize, nodes: &[MenuNode]) -> Vec<MenuLine<'a, M>>
    where
        M: 'a,
    {
        nodes
            .iter()
            .flat_map(|node| self.node_lines(depth, node))
            .collect()
    }

    fn node_lines<'a>(&self, depth: usize, node: &MenuNode) -> Vec<MenuLine<'a, M>>
    where
        M: 'a,
    {
        match node.kind {
            MenuKind::Separator => vec![MenuLine::Divider],
            MenuKind::Submenu => {
                let open = self.open.contains(&node.id);
                let head = self.row(
                    depth,
                    node,
                    Trailing::Submenu { open },
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
                vec![self.row(depth, node, Trailing::Nothing, (self.on_activate)(node.id))]
            }
        }
    }

    fn row<'a>(
        &self,
        depth: usize,
        node: &MenuNode,
        trailing: Trailing,
        on_press: M,
    ) -> MenuLine<'a, M>
    where
        M: 'a,
    {
        let icon_size = self.config.theme.font.icon_size_px;
        let color = self.config.theme.palette.bar_foreground;
        let leading = node
            .icon
            .as_ref()
            .map(|source| IconHandle::from(source).view(icon_size, 1.0))
            .or_else(|| {
                matches!(node.kind, MenuKind::Toggle(true))
                    .then(|| Glyph::from(Phosphor::Check(Weight::Regular)).view(icon_size, color))
            });
        MenuLine::Row(MenuRow {
            leading,
            label: node.label.clone(),
            trailing,
            enabled: node.enabled,
            depth,
            on_press: Some(on_press),
        })
    }
}
