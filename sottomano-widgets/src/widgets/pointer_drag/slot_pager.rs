use std::hash::Hash;

use corredo::pointer_drag::slot_pager::slot_pager as generic_slot_pager;
pub use corredo::pointer_drag::slot_pager::{
    PagerEdge, SlotAlarm, SlotPager, UngroupedHighlightLook,
};
use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position};

fn pager_edge_of(position: Position) -> PagerEdge {
    match position {
        Position::Top => PagerEdge::Top,
        Position::Bottom => PagerEdge::Bottom,
        Position::Left => PagerEdge::Left,
    }
}

pub fn slot_pager<'a, Message: 'a, Key: Copy + Eq + Hash + 'static>(
    slots: impl IntoIterator<Item = (Key, Element<'a, Message>)>,
    edge: Position,
    config: &Config,
) -> SlotPager<'a, Message, Key> {
    generic_slot_pager(slots, pager_edge_of(edge))
        .indicator_motion(config.desktop_panels.slot_drag.focus_indicator_duration)
        .group_gap(config.theme.spacing.item_gap)
}
