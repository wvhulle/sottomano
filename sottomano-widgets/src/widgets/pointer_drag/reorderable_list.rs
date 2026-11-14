use std::hash::Hash;

use corredo::{
    drag_reorder::{ReorderableList, reorderable_list as generic_slot_pager},
    orientation::Edge,
};
use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position};

fn pager_edge_of(position: Position) -> Edge {
    match position {
        Position::Top => Edge::Top,
        Position::Bottom => Edge::Bottom,
        Position::Left => Edge::Left,
    }
}

pub fn reorderable_list<'a, Message: 'a, Key: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (Key, Element<'a, Message>)>,
    edge: Position,
    config: &Config,
) -> ReorderableList<'a, Message, Key> {
    generic_slot_pager(slots, pager_edge_of(edge))
        .indicator_motion(config.desktop_panels.slot_drag.focus_indicator_duration)
        .group_gap(config.theme.spacing.item_gap)
}
