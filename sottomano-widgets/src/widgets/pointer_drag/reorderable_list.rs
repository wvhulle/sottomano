use std::hash::Hash;

use corredo::drag_reorder::{ReorderableList, reorderable_list as generic_slot_pager};
use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position};

use crate::widgets::screen_edge::screen_edge_of;

pub fn reorderable_list<'a, Message: 'a, Key: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (Key, Element<'a, Message>)>,
    edge: Position,
    config: &Config,
) -> ReorderableList<'a, Message, Key> {
    generic_slot_pager(slots, screen_edge_of(edge))
        .indicator_motion(config.desktop_panels.slot_drag.focus_indicator_duration)
        .group_gap(config.theme.spacing.item_gap)
}
